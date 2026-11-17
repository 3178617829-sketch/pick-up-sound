//! 外部歌单导入（网易云音乐 / QQ 音乐 / 酷狗音乐 → B站歌单）
//!
//! 流程分两步，和「导入 B站收藏夹」保持一致的交互：
//! 1. `import_playlist_fetch`：粘贴歌单链接 → 识别平台 → 拉取歌单元信息与曲目；
//! 2. `import_playlist_match`：逐首在 B站搜索，给每首挑出若干候选并按匹配度排序；
//! 用户确认候选后由前端调用既有的 `create_imported_playlist` 落库。
//!
//! 注意：B站没有那三家的版权曲库，这里做的是「歌名 + 歌手 → B站对应视频」的
//! 语义迁移，不是音源搬运。匹配不中的条目由用户手动挑选或跳过。

use crate::search::{SearchClient, SearchVideo};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Semaphore;

/// 单次导入的曲目上限，与 `create_imported_playlist` 的 200 条限制一致。
const MAX_TRACKS: usize = 200;
/// 匹配时的并发数：B站搜索接口对高频请求敏感，3 路并发实测稳定。
const MATCH_CONCURRENCY: usize = 3;
/// 每首匹配之间的固定间隔，压住触发 412 风控的概率。
const MATCH_INTERVAL_MS: u64 = 150;
/// 三家的接口都按浏览器 UA 走，写一处省得三份不一样。
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Platform {
    Netease,
    Qq,
    Kugou,
}

impl Platform {
    fn key(self) -> &'static str {
        match self {
            Self::Netease => "netease",
            Self::Qq => "qq",
            Self::Kugou => "kugou",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Netease => "网易云音乐",
            Self::Qq => "QQ 音乐",
            Self::Kugou => "酷狗音乐",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportedTrack {
    pub(crate) index: usize,
    pub(crate) name: String,
    pub(crate) artist: String,
    pub(crate) duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportedPlaylist {
    pub(crate) platform: String,
    pub(crate) platform_label: String,
    pub(crate) playlist_id: String,
    pub(crate) name: String,
    pub(crate) tracks: Vec<ImportedTrack>,
    /// 被截断到上限时提示前端
    pub(crate) truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportTrackInput {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) artist: String,
    #[serde(default)]
    pub(crate) duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MatchCandidate {
    pub(crate) bvid: String,
    pub(crate) title: String,
    pub(crate) uploader: String,
    pub(crate) thumbnail_url: String,
    pub(crate) duration_seconds: u64,
    pub(crate) play_count: Option<u64>,
    pub(crate) score: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackMatch {
    pub(crate) index: usize,
    pub(crate) name: String,
    pub(crate) artist: String,
    pub(crate) duration_ms: u64,
    pub(crate) candidates: Vec<MatchCandidate>,
    pub(crate) error: Option<String>,
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build playlist import http client")
    })
}

/// 从用户粘贴的内容里认出平台和歌单 ID。
/// 支持完整链接、带井号的分享链接、以及纯数字 ID（纯数字默认按网易云处理）。
fn detect_target(input: &str, platform_hint: Option<&str>) -> Result<(Platform, String), String> {
    let text = input.trim();
    if text.is_empty() {
        return Err("请粘贴歌单链接。".to_owned());
    }
    let lower = text.to_lowercase();

    let platform = if lower.contains("music.163.com") || lower.contains("163cn.tv") {
        Platform::Netease
    } else if lower.contains("y.qq.com") || lower.contains("qq.com") {
        Platform::Qq
    } else if lower.contains("kugou.com") {
        Platform::Kugou
    } else {
        match platform_hint.map(|value| value.trim().to_lowercase()) {
            Some(value) if value == "qq" => Platform::Qq,
            Some(value) if value == "kugou" => Platform::Kugou,
            Some(value) if value == "netease" || value.is_empty() => Platform::Netease,
            Some(other) => return Err(format!("不认识的平台：{other}")),
            // 没给提示也没域名：按网易云试，用户在界面上可以手动切平台
            None => Platform::Netease,
        }
    };

    let id = extract_numeric_id(&lower)
        .ok_or_else(|| "没能从链接里认出歌单 ID，请确认复制的是歌单分享链接。".to_owned())?;
    Ok((platform, id))
}

/// 从链接里取歌单 ID。
/// 手机端分享出来的网易云链接长这样：
/// `https://music.163.com/#/playlist?app_version=9.6.15&id=14383331311&userid=1438333&...`
/// 里面数字一大堆，取「最长数字串」会抓到 userid 或版本号，所以先认显式的 id 参数；
/// 认不出来（例如 QQ 的 `/playlist/7207147475`）再退回最长数字串。
fn extract_numeric_id(text: &str) -> Option<String> {
    const KEYS: [&str; 5] = ["id", "listid", "disstid", "specialid", "playlistid"];
    for segment in text.split(['?', '&', '#', '/', ';']) {
        let Some((key, value)) = segment.split_once('=') else {
            continue;
        };
        if !KEYS.contains(&key) {
            continue;
        }
        let digits: String = value.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        if digits.len() >= 5 {
            return Some(digits);
        }
    }
    let mut best = String::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            if current.len() > best.len() {
                best = current.clone();
            }
        } else {
            current.clear();
        }
    }
    if best.len() >= 5 {
        Some(best)
    } else {
        None
    }
}

fn strip_tags(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_owned()
}

fn json_path<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    Some(cursor)
}

fn json_array(value: &serde_json::Value, key: &str) -> Vec<serde_json::Value> {
    value
        .get(key)
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default()
}

fn json_str(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    json_path(value, path)
        .and_then(|item| item.as_str())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn json_u64(value: &serde_json::Value, path: &[&str]) -> Option<u64> {
    json_path(value, path).and_then(|item| {
        item.as_u64()
            .or_else(|| item.as_f64().map(|number| number.max(0.0).round() as u64))
            .or_else(|| item.as_str().and_then(|text| text.parse::<u64>().ok()))
    })
}

/// 网易云：`/api/playlist/detail` 免登录返回歌单名与曲目。
/// 超过 1000 首的大歌单这个接口只会给前一部分，够用且不额外签名。
async fn fetch_netease(id: &str) -> Result<ImportedPlaylist, String> {
    // v6 接口匿名就能读；老接口对一部分歌单（手机端分享的「我喜欢的音乐」等）
    // 直接返回 code=20001「需要登录」，所以先试 v6、失败再退回老接口。
    if let Ok(body) = netease_json(&format!(
        "https://music.163.com/api/v6/playlist/detail?id={id}&n=1000&s=0"
    ))
    .await
    {
        let code = body
            .get("code")
            .and_then(|item| item.as_i64())
            .unwrap_or(200);
        if code == 200 {
            if let Some(playlist) = body.get("playlist") {
                return netease_from_v6(id, playlist).await;
            }
        }
    }
    let body = netease_json(&format!(
        "https://music.163.com/api/playlist/detail?id={id}"
    ))
    .await?;
    let code = body
        .get("code")
        .and_then(|item| item.as_i64())
        .unwrap_or(200);
    if code != 200 {
        return Err(format!(
            "网易云接口返回 code={code}，可能是歌单不存在或需要登录。"
        ));
    }
    let name = json_str(&body, &["result", "name"]).unwrap_or_else(|| format!("网易云歌单 {id}"));
    let tracks_value = json_path(&body, &["result", "tracks"])
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(pack_netease(
        id,
        name,
        collect_netease_tracks(&tracks_value),
    ))
}

/// v6 接口：`playlist.tracks` 只给首屏十几首，完整列表在 `trackIds` 里，要按 id 再取一遍。
async fn netease_from_v6(
    id: &str,
    playlist: &serde_json::Value,
) -> Result<ImportedPlaylist, String> {
    let name = json_str(playlist, &["name"]).unwrap_or_else(|| format!("网易云歌单 {id}"));
    let track_ids: Vec<u64> = playlist
        .get("trackIds")
        .and_then(|item| item.as_array())
        .map(|ids| {
            ids.iter()
                .filter_map(|item| json_u64(item, &["id"]))
                .collect()
        })
        .unwrap_or_default();
    let inline = json_path(playlist, &["tracks"])
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let tracks = if track_ids.len() > inline.len() {
        netease_song_details(&track_ids).await?
    } else {
        collect_netease_tracks(&inline)
    };
    Ok(pack_netease(id, name, tracks))
}

/// 按 id 批量补歌名/歌手/时长。`ids` 得写成 JSON 数组的样子（`%5B`/`%5D` 是方括号），
/// 一次 100 个，实测 200 个会有条目被悄悄丢掉。
async fn netease_song_details(track_ids: &[u64]) -> Result<Vec<ImportedTrack>, String> {
    let mut tracks = Vec::new();
    for chunk in track_ids.chunks(100) {
        if tracks.len() >= MAX_TRACKS {
            break;
        }
        let list = chunk
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let body = netease_json(&format!(
            "https://music.163.com/api/song/detail/?ids=%5B{list}%5D&csrf_token="
        ))
        .await?;
        tracks.extend(collect_netease_tracks(&json_array(&body, "songs")));
    }
    Ok(tracks)
}

/// 网易云单曲字段：老接口（含 `/api/song/detail`）是 `artists[].name` + `duration`，
/// v6 是 `ar[].name` + `dt`，两套都要认。
fn netease_track(item: &serde_json::Value) -> Option<ImportedTrack> {
    let name = json_str(item, &["name"])?;
    let artist = item
        .get("ar")
        .or_else(|| item.get("artists"))
        .and_then(|item| item.as_array())
        .map(|artists| {
            artists
                .iter()
                .filter_map(|artist| artist.get("name").and_then(|name| name.as_str()))
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .unwrap_or_default();
    let duration_ms = json_u64(item, &["dt"])
        .or_else(|| json_u64(item, &["duration"]))
        .unwrap_or(0);
    Some(ImportedTrack {
        index: 0,
        name,
        artist,
        duration_ms,
    })
}

fn collect_netease_tracks(items: &[serde_json::Value]) -> Vec<ImportedTrack> {
    let mut tracks: Vec<ImportedTrack> = items.iter().filter_map(netease_track).collect();
    for (index, track) in tracks.iter_mut().enumerate() {
        track.index = index;
    }
    tracks
}

fn pack_netease(id: &str, name: String, mut tracks: Vec<ImportedTrack>) -> ImportedPlaylist {
    let truncated = tracks.len() > MAX_TRACKS;
    tracks.truncate(MAX_TRACKS);
    ImportedPlaylist {
        platform: Platform::Netease.key().to_owned(),
        platform_label: Platform::Netease.label().to_owned(),
        playlist_id: id.to_owned(),
        name,
        tracks,
        truncated,
    }
}

async fn netease_json(url: &str) -> Result<serde_json::Value, String> {
    let response = http_client()
        .get(url)
        .header(reqwest::header::REFERER, "https://music.163.com/")
        .header(reqwest::header::USER_AGENT, BROWSER_UA)
        .send()
        .await
        .map_err(|error| format!("请求网易云失败：{error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("网易云接口返回 HTTP {status}"));
    }
    response
        .json()
        .await
        .map_err(|error| format!("网易云返回内容无法解析：{error}"))
}

/// QQ 音乐：`fcg_ucc_getcdinfo_byids_cp` 免登录返回 `cdlist[0]`。
async fn fetch_qq(id: &str) -> Result<ImportedPlaylist, String> {
    let url = format!(
        "https://c.y.qq.com/qzone/fcg-bin/fcg_ucc_getcdinfo_byids_cp.fcg\
         ?type=1&json=1&utf8=1&onlysong=0&disstid={id}&format=json&platform=yqq.json&needNewCode=0"
    );
    let response = http_client()
        .get(&url)
        .header(reqwest::header::REFERER, "https://y.qq.com/")
        .header(reqwest::header::USER_AGENT, BROWSER_UA)
        .send()
        .await
        .map_err(|error| format!("请求 QQ 音乐失败：{error}"))?;
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("QQ 音乐返回内容无法解析：{error}"))?;
    let cdlist = body
        .get("cdlist")
        .and_then(|item| item.as_array())
        .and_then(|list| list.first())
        .ok_or_else(|| "QQ 音乐没有返回歌单信息，可能是歌单 ID 不对或歌单未公开。".to_owned())?;

    let name = json_str(cdlist, &["dissname"]).unwrap_or_else(|| format!("QQ 音乐歌单 {id}"));
    let songlist = cdlist
        .get("songlist")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();

    let mut tracks = Vec::new();
    for item in &songlist {
        let track_name = json_str(item, &["songname"]).or_else(|| json_str(item, &["title"]));
        let Some(track_name) = track_name else {
            continue;
        };
        let artist = item
            .get("singer")
            .and_then(|item| item.as_array())
            .map(|singers| {
                singers
                    .iter()
                    .filter_map(|singer| singer.get("name").and_then(|name| name.as_str()))
                    .collect::<Vec<_>>()
                    .join(" / ")
            })
            .unwrap_or_default();
        // QQ 的 interval 单位是秒
        let duration_ms = json_u64(item, &["interval"]).unwrap_or(0) * 1000;
        tracks.push(ImportedTrack {
            index: tracks.len(),
            name: track_name,
            artist,
            duration_ms,
        });
    }

    let total = tracks.len();
    let truncated = total > MAX_TRACKS;
    tracks.truncate(MAX_TRACKS);
    Ok(ImportedPlaylist {
        platform: Platform::Qq.key().to_owned(),
        platform_label: Platform::Qq.label().to_owned(),
        playlist_id: id.to_owned(),
        name,
        tracks,
        truncated,
    })
}

/// 酷狗：`mobilecdn / special/song` 匿名分页返回曲目，歌单名另走 `special/info`。
async fn fetch_kugou(id: &str) -> Result<ImportedPlaylist, String> {
    let mut name = String::new();
    let info_url = format!(
        "http://mobilecdn.kugou.com/api/v3/special/info?specialid={id}&version=9108&plat=0"
    );
    if let Ok(response) = http_client()
        .get(&info_url)
        .header(reqwest::header::USER_AGENT, BROWSER_UA)
        .send()
        .await
    {
        if let Ok(body) = response.json::<serde_json::Value>().await {
            if let Some(value) = json_str(&body, &["data", "specialname"])
                .or_else(|| json_str(&body, &["data", "name"]))
            {
                name = value;
            }
        }
    }
    if name.is_empty() {
        name = format!("酷狗歌单 {id}");
    }

    let mut tracks = Vec::new();
    let mut page = 1u32;
    loop {
        let url = format!(
            "http://mobilecdn.kugou.com/api/v3/special/song?specialid={id}\
             &page={page}&pagesize=300&version=9108&plat=0"
        );
        let response = http_client()
            .get(&url)
            .header(reqwest::header::USER_AGENT, BROWSER_UA)
            .send()
            .await
            .map_err(|error| format!("请求酷狗失败：{error}"))?;
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|error| format!("酷狗返回内容无法解析：{error}"))?;
        let status = body
            .get("status")
            .and_then(|item| item.as_i64())
            .unwrap_or(0);
        if status != 1 {
            return Err("酷狗没有返回歌单内容，可能是歌单 ID 不对或歌单未公开。".to_owned());
        }
        let songs = body
            .get("data")
            .and_then(|item| item.get("info"))
            .and_then(|item| item.as_array())
            .cloned()
            .unwrap_or_default();
        if songs.is_empty() {
            break;
        }
        for item in &songs {
            let raw_name = json_str(item, &["filename"]).or_else(|| json_str(item, &["songname"]));
            let Some(raw_name) = raw_name else { continue };
            let cleaned = strip_tags(&raw_name);
            if cleaned.is_empty() {
                continue;
            }
            // 酷狗歌单接口的 singername 常年为空，歌手与歌名挤在 filename 里（「歌手 - 歌名」），
            // 只有确实存在单独歌手字段时才用它，否则从 filename 拆。
            let explicit_artist = json_str(item, &["singername"])
                .or_else(|| json_str(item, &["singer_name"]))
                .unwrap_or_default();
            let (track_name, artist) = match cleaned.split_once(" - ") {
                Some((left, right)) if explicit_artist.is_empty() && !left.trim().is_empty() => {
                    (right.trim().to_owned(), left.trim().to_owned())
                }
                _ => (cleaned, explicit_artist),
            };
            let duration_ms = json_u64(item, &["duration"]).unwrap_or(0) * 1000;
            tracks.push(ImportedTrack {
                index: tracks.len(),
                name: track_name,
                artist,
                duration_ms,
            });
            if tracks.len() >= MAX_TRACKS {
                break;
            }
        }
        if tracks.len() >= MAX_TRACKS {
            break;
        }
        page += 1;
        if page > 20 {
            break;
        }
    }

    let truncated = tracks.len() >= MAX_TRACKS;
    Ok(ImportedPlaylist {
        platform: Platform::Kugou.key().to_owned(),
        platform_label: Platform::Kugou.label().to_owned(),
        playlist_id: id.to_owned(),
        name,
        tracks,
        truncated,
    })
}

#[tauri::command]
pub(crate) async fn import_playlist_fetch(
    url: String,
    platform: Option<String>,
) -> Result<ImportedPlaylist, String> {
    let (platform, id) = detect_target(&url, platform.as_deref())?;
    let playlist = match platform {
        Platform::Netease => fetch_netease(&id).await?,
        Platform::Qq => fetch_qq(&id).await?,
        Platform::Kugou => fetch_kugou(&id).await?,
    };
    if playlist.tracks.is_empty() {
        return Err("这个歌单里没有读到任何歌曲，换一个链接试试。".to_owned());
    }
    Ok(playlist)
}

/// 文本归一化：去掉空白与各种标点，避免「晴天 - 周杰伦」和「晴天-周杰伦」对不上。
fn normalize_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_whitespace() {
            continue;
        }
        let skip = matches!(
            ch,
            '(' | ')'
                | '['
                | ']'
                | '（'
                | '）'
                | '【'
                | '】'
                | '《'
                | '》'
                | '、'
                | '，'
                | ','
                | '。'
                | '.'
                | '·'
                | '-'
                | '_'
                | '—'
                | '～'
                | '~'
                | '!'
                | '！'
                | '?'
                | '？'
                | ':'
                | '：'
                | '\''
                | '"'
                | '“'
                | '”'
                | '/'
                | '／'
        );
        if skip {
            continue;
        }
        for lower in ch.to_lowercase() {
            out.push(lower);
        }
    }
    out
}

/// 去掉歌名里的括号补充信息，只留主体，方便在 B站标题里找。
fn clean_track_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut depth = 0usize;
    for ch in name.chars() {
        match ch {
            '(' | '（' | '[' | '【' | '《' => depth += 1,
            ')' | '）' | ']' | '】' | '》' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    let cleaned = out.trim().to_owned();
    if cleaned.is_empty() {
        name.trim().to_owned()
    } else {
        cleaned
    }
}

/// 二元组覆盖率：needle 的相邻字对有多少出现在 haystack 里。
fn bigram_overlap(needle: &str, haystack: &str) -> f64 {
    let chars: Vec<char> = needle.chars().collect();
    if chars.is_empty() {
        return 0.0;
    }
    if chars.len() == 1 {
        return if haystack.contains(needle) { 1.0 } else { 0.0 };
    }
    let mut hit = 0usize;
    let total = chars.len() - 1;
    for window in chars.windows(2) {
        let gram: String = window.iter().collect();
        if haystack.contains(&gram) {
            hit += 1;
        }
    }
    hit as f64 / total as f64
}

fn duration_score(expected_ms: u64, actual_seconds: u64) -> f64 {
    if expected_ms == 0 || actual_seconds == 0 {
        return 0.5;
    }
    let diff = (expected_ms as i64 - actual_seconds as i64 * 1000).abs();
    if diff <= 8_000 {
        1.0
    } else if diff <= 20_000 {
        0.75
    } else if diff <= 45_000 {
        0.4
    } else {
        0.0
    }
}

fn play_score(play_count: Option<u64>) -> f64 {
    match play_count {
        Some(count) => (((count as f64) + 1.0).log10() / 7.0).clamp(0.0, 1.0),
        None => 0.0,
    }
}

/// 歌单场景专用打分：歌名 45% + 歌手 25% + 时长 25% + 播放量 5%，并对合集/翻唱这类噪声降权。
///
/// 不改 `search::rerank_search_results`——那是给普通搜索用的，加了歌手权重反而会让
/// 「搜歌名」的结果变差。
fn score_candidate(track: &ImportTrackInput, video: &SearchVideo) -> f64 {
    let title = normalize_text(&video.title);
    let uploader = normalize_text(&video.uploader);
    let name_clean = normalize_text(&clean_track_name(&track.name));
    let name_raw = normalize_text(&track.name);
    let artist = normalize_text(&track.artist);

    let name_score = if !name_clean.is_empty() && title.contains(&name_clean) {
        1.0
    } else if !name_raw.is_empty() && title.contains(&name_raw) {
        1.0
    } else {
        bigram_overlap(&name_clean, &title).max(bigram_overlap(&name_raw, &title)) * 0.8
    };

    let artist_score = if artist.is_empty() {
        0.6
    } else if title.contains(&artist) || uploader.contains(&artist) {
        1.0
    } else {
        bigram_overlap(&artist, &title).max(bigram_overlap(&artist, &uploader)) * 0.7
    };

    let dur = duration_score(track.duration_ms, video.duration_seconds);
    let play = play_score(video.play_count);

    let mut score = name_score * 0.45 + artist_score * 0.25 + dur * 0.25 + play * 0.05;

    let noise = [
        "合集",
        "串烧",
        "1小时",
        "一小时",
        "纯音乐",
        "伴奏",
        "翻唱",
        "鬼畜",
        "钢琴",
        "dj",
        "remix",
        "cover",
        "歌单",
        "循环",
        "完整版",
    ];
    if noise.iter().any(|marker| title.contains(marker)) {
        score *= 0.55;
    }
    if !name_clean.is_empty() && !title.contains(&name_clean) {
        score *= 0.85;
    }
    score
}

/// 单首曲目：搜索 + 打分 + 取前若干个候选。
async fn match_one(
    search: &SearchClient,
    track: &ImportTrackInput,
    limit: usize,
) -> Result<Vec<MatchCandidate>, String> {
    let mut keyword = format!("{} {}", clean_track_name(&track.name), track.artist)
        .trim()
        .to_owned();
    if keyword.is_empty() {
        return Err("歌名为空，无法搜索。".to_owned());
    }

    let mut videos = search
        .search_videos_page(&keyword, 1, None, None, None, true)
        .await?;

    // 带歌手搜不到东西时退回只搜歌名——比直接判失败有用的多。
    if videos.len() < 3 {
        keyword = clean_track_name(&track.name);
        if !keyword.is_empty() {
            if let Ok(retry) = search
                .search_videos_page(&keyword, 1, None, None, None, true)
                .await
            {
                let mut seen: Vec<String> = videos.iter().map(|item| item.bvid.clone()).collect();
                for item in retry {
                    if !seen.iter().any(|bvid| bvid == &item.bvid) {
                        seen.push(item.bvid.clone());
                        videos.push(item);
                    }
                }
            }
        }
    }

    let mut scored: Vec<(f64, SearchVideo)> = videos
        .into_iter()
        .map(|video| (score_candidate(track, &video), video))
        .collect();
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));

    Ok(scored
        .into_iter()
        .take(limit)
        .map(|(score, video)| MatchCandidate {
            bvid: video.bvid,
            title: video.title,
            uploader: video.uploader,
            thumbnail_url: video.thumbnail_url,
            duration_seconds: video.duration_seconds,
            play_count: video.play_count,
            score,
        })
        .collect())
}

#[tauri::command]
pub(crate) async fn import_playlist_match(
    state: tauri::State<'_, AppState>,
    tracks: Vec<ImportTrackInput>,
    candidate_limit: Option<usize>,
) -> Result<Vec<TrackMatch>, String> {
    if tracks.is_empty() {
        return Ok(Vec::new());
    }
    if tracks.len() > MAX_TRACKS {
        return Err(format!("一次最多匹配 {MAX_TRACKS} 首。"));
    }

    let limit = candidate_limit.unwrap_or(5).clamp(1, 10);
    let search = state.search.clone();
    let semaphore = Arc::new(Semaphore::new(MATCH_CONCURRENCY));
    let mut join_set = tokio::task::JoinSet::new();

    for (index, track) in tracks.into_iter().enumerate() {
        let search = search.clone();
        let semaphore = semaphore.clone();
        join_set.spawn(async move {
            let _permit = match semaphore.acquire().await {
                Ok(permit) => permit,
                Err(_) => {
                    return TrackMatch {
                        index,
                        name: track.name.clone(),
                        artist: track.artist.clone(),
                        duration_ms: track.duration_ms,
                        candidates: Vec::new(),
                        error: Some("匹配任务被取消。".to_owned()),
                    }
                }
            };
            let outcome = match_one(&search, &track, limit).await;
            tokio::time::sleep(Duration::from_millis(MATCH_INTERVAL_MS)).await;
            match outcome {
                Ok(candidates) => TrackMatch {
                    index,
                    name: track.name,
                    artist: track.artist,
                    duration_ms: track.duration_ms,
                    candidates,
                    error: None,
                },
                Err(error) => TrackMatch {
                    index,
                    name: track.name,
                    artist: track.artist,
                    duration_ms: track.duration_ms,
                    candidates: Vec::new(),
                    error: Some(error),
                },
            }
        });
    }

    let mut results: Vec<TrackMatch> = Vec::new();
    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(result) => results.push(result),
            Err(error) => {
                return Err(format!("匹配任务异常终止：{error}"));
            }
        }
    }
    results.sort_by_key(|item| item.index);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(title: &str, uploader: &str, seconds: u64, plays: Option<u64>) -> SearchVideo {
        SearchVideo {
            bvid: "BV1xx411c7mD".to_owned(),
            title: title.to_owned(),
            uploader: uploader.to_owned(),
            thumbnail_url: String::new(),
            duration_seconds: seconds,
            play_count: plays,
            pubdate: None,
        }
    }

    #[test]
    fn extracts_ids_from_share_links() {
        assert_eq!(
            extract_numeric_id("https://music.163.com/playlist?id=3778678&userid=1").as_deref(),
            Some("3778678")
        );
        assert_eq!(
            extract_numeric_id("https://y.qq.com/n/ryqq/playlist/8666892546").as_deref(),
            Some("8666892546")
        );
        assert_eq!(
            extract_numeric_id("https://www.kugou.com/yy/special/single/123456.html").as_deref(),
            Some("123456")
        );
        // 手机端分享的网易云链接：id 后面还挂着更长的 userid，不能被它带偏。
        assert_eq!(
            extract_numeric_id(
                "https://music.163.com/#/playlist?app_version=9.6.15&id=14383331311&userid=143833313&_time=1698"
            )
            .as_deref(),
            Some("14383331311")
        );
        assert_eq!(
            extract_numeric_id("https://y.qq.com/n/ryqq/playlist?disstid=8666892546&star=1")
                .as_deref(),
            Some("8666892546")
        );
    }

    #[test]
    fn detects_platforms_from_hosts() {
        assert_eq!(
            detect_target("https://music.163.com/playlist?id=123456", None)
                .unwrap()
                .0,
            Platform::Netease
        );
        assert_eq!(
            detect_target("https://y.qq.com/n/ryqq/playlist/123456", None)
                .unwrap()
                .0,
            Platform::Qq
        );
        assert_eq!(
            detect_target("https://www.kugou.com/yy/special/single/123456.html", None)
                .unwrap()
                .0,
            Platform::Kugou
        );
        assert_eq!(
            detect_target("123456", Some("kugou")).unwrap().0,
            Platform::Kugou
        );
    }

    #[test]
    fn cleans_bracketed_suffixes() {
        assert_eq!(clean_track_name("晴天 (Live)"), "晴天");
        assert_eq!(clean_track_name("起风了【纯音乐】"), "起风了");
        assert_eq!(clean_track_name("（纯音乐）"), "（纯音乐）");
    }

    #[test]
    fn exact_title_beats_noise() {
        let track = ImportTrackInput {
            name: "晴天".to_owned(),
            artist: "周杰伦".to_owned(),
            duration_ms: 269_000,
        };
        let exact = video("周杰伦《晴天》官方MV", "周杰伦", 269, Some(9_000_000));
        let noise = video("晴天合集 1小时循环", "某搬运", 3_600, Some(2_000_000));
        assert!(score_candidate(&track, &exact) > score_candidate(&track, &noise));
    }

    #[test]
    fn duration_mismatch_lowers_score() {
        let track = ImportTrackInput {
            name: "晴天".to_owned(),
            artist: "周杰伦".to_owned(),
            duration_ms: 269_000,
        };
        let near = video("晴天", "周杰伦", 270, Some(1000));
        let far = video("晴天", "周杰伦", 60, Some(1000));
        assert!(score_candidate(&track, &near) > score_candidate(&track, &far));
    }
}
