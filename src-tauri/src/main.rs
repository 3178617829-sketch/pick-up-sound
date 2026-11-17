#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod appearance;
mod audio_cache;
mod bar_commands;
mod bar_layout;
mod bindings;
mod build_info;
mod commands;
mod fav_import;
mod guest_playurl;
mod library;
mod loudness;
mod lyrics;
mod main_window;
mod mini_player;
mod native_bar;
mod playlist_import;
mod proxy;
mod ranking;
mod resolve;
mod search;
mod shortcuts;
mod state;
mod storage;
mod tray;
mod tray_status;
mod wbi;
mod ytdlp_adapter;

use axum::routing::get;
use axum::Router;
use bilibili_music_core::{bilibili_cookie_path, yt_dlp_path};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

use appearance::{choose_background_image, load_background_image};
use guest_playurl::GuestPlayurlClient;
use loudness::analyze_track_loudness;
use proxy::{bind_browser_safe_loopback, build_proxy_client, proxy_audio, ProxyState};
use ranking::{RankingClient, RankingTrack};
use resolve::ResolveCoordinator;
use search::SearchClient;
use state::{AppState, StreamSource};

/// 单实例锁：用临时目录下的文件独占锁，拿不到就说明已有实例在跑。
/// 没有它，用户双击两次就会有两个进程各开一条任务栏条，互相把对方顶掉。
fn acquire_instance_lock() -> Option<std::fs::File> {
    let path = std::env::temp_dir().join("pick-up-sound-instance.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .ok()?;
    match file.try_lock() {
        Ok(()) => Some(file),
        Err(_) => None,
    }
}

/// 程序入口。按顺序做六件事，每段开头都有一句注释说明它为什么存在：
///
/// 1. 拿单实例锁——拿不到说明已经有一个在跑，直接退出（两个进程会抢同一个缓存目录）；
/// 2. 绑本地音频代理的端口（随机端口，避开浏览器对非常用端口的拦截）；
/// 3. 组装代理状态和四个共享客户端（游客播放地址、搜索、排行、收藏导入）；
/// 4. debug 构建下把 bar 命令的类型绑定导出到 `ui/shared/bindings/bar.ts`；
/// 5. 组装 `tauri::Builder`：注册全局状态，setup 里装 watchdog / 托盘 / 任务栏条 / 快捷键 / 音频代理路由；
/// 6. 注册全部 IPC 命令，进事件循环。
///
/// 这个函数 270 多行，八成是第 6 段那张命令清单（四十多条）撑起来的。
/// 拆成几个小函数反而要在几处之间跳着看，所以保持一整块、只用注释分段。
fn main() {
    // 拿不到实例锁说明已经有一个在跑了，直接退出
    let _instance_lock = match acquire_instance_lock() {
        Some(lock) => lock,
        None => std::process::exit(0),
    };

    let listener = bind_browser_safe_loopback().expect("failed to bind the local audio proxy");
    listener
        .set_nonblocking(true)
        .expect("failed to configure the local audio proxy");
    let port = listener
        .local_addr()
        .expect("failed to read the local audio proxy address")
        .port();

    let proxy = ProxyState {
        client: build_proxy_client().expect("failed to create the audio proxy client"),
        streams: Arc::new(RwLock::new(HashMap::new())),
    };
    let server_proxy = proxy.clone();
    let cookie_path = bilibili_cookie_path();
    let yt_dlp = yt_dlp_path();
    eprintln!(
        "[runtime] cwd={} cookie={} yt-dlp={}",
        std::env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("<unavailable: {error}>")),
        cookie_path.display(),
        yt_dlp.display()
    );
    let guest =
        Arc::new(GuestPlayurlClient::new().expect("failed to create the guest playurl client"));
    let search = SearchClient::new(cookie_path, Arc::clone(&guest))
        .expect("failed to create the search client");
    let ranking =
        RankingClient::new(Arc::clone(&guest)).expect("failed to create the ranking client");
    let favorite_import = fav_import::FavoriteImportClient::new(Arc::clone(&guest))
        .expect("failed to create the favorite import client");

    // 开发期把 bar 命令的类型绑定导出到 ui/shared/bindings/bar.ts。
    // release 不编译这段：包里没有导出步骤，启动路径上也不该写文件。
    #[cfg(debug_assertions)]
    if let Err(error) = bindings::export_bar_bindings() {
        eprintln!("[bindings] 导出 TypeScript 绑定失败：{error}");
    }

    tauri::Builder::default()
        .manage(std::sync::Mutex::new(main_window::ReadyGate::default()))
        .on_window_event(|window, event| {
            if window.label() == mini_player::MINI_WINDOW_LABEL
                && matches!(event, tauri::WindowEvent::CloseRequested { .. })
            {
                if let Err(error) = mini_player::restore_main_window(window.app_handle()) {
                    eprintln!("[mini-player] close recovery failed: {error}");
                }
            }
        })
        .manage(AppState {
            loudness_busy: Arc::new(AtomicBool::new(false)),
            cache_busy: Arc::new(AtomicBool::new(false)),
            proxy,
            proxy_base_url: format!("http://127.0.0.1:{port}"),
            search,
            ranking,
            favorite_import,
            ranking_cache: Arc::new(RwLock::new(None)),
            guest,
            collection_cache: Arc::new(RwLock::new(None)),
            resolver: Arc::new(ResolveCoordinator::default()),
            stream_source: Arc::new(RwLock::new(StreamSource::Guest)),
        })
        .setup(move |app| {
            main_window::install_watchdog(app.handle().clone());
            tray_status::install(app);
            native_bar::install(app.handle().clone());
            tray::install(app.handle());
            shortcuts::install(app);
            // 注意：这里曾经在第 1.5 秒无条件打开任务栏条，完全不管用户上次选的是
            // 「任务栏」还是「小组件」——结果启动出来的是条，而界面上的高亮是小组件，
            // 用户看到的就是「对不上 / 乱」。现在改由前端读 mini-surface-mode 后自己决定。
            tauri::async_runtime::spawn(async move {
                let listener = tokio::net::TcpListener::from_std(listener)
                    .expect("failed to start the local audio proxy listener");
                let router = Router::new()
                    .route("/audio/{token}", get(proxy_audio).head(proxy_audio))
                    .with_state(server_proxy);
                if let Err(error) = axum::serve(listener, router).await {
                    eprintln!("local audio proxy stopped: {error}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            build_info::build_stamp,
            main_window::main_window_ready,
            main_window::quit_app,
            main_window::show_main_window_cmd,
            fav_import::read_public_favorite_page,
            library::playlists::create_imported_playlist,
            playlist_import::import_playlist_fetch,
            playlist_import::import_playlist_match,
            tray_status::set_taskbar_playback_state,
            mini_player::open_mini_player,
            mini_player::mini_player_ready,
            mini_player::exit_mini_player,
            mini_player::set_mini_scale,
            bar_commands::open_taskbar_bar,
            bar_commands::close_taskbar_bar,
            bar_commands::taskbar_bar_visible,
            bar_commands::push_bar_state,
            bar_commands::bar_control,
            resolve::prepare_audio,
            analyze_track_loudness,
            audio_cache::cache_track_audio,
            audio_cache::get_audio_cache_settings,
            audio_cache::set_audio_cache_settings,
            audio_cache::get_audio_cache_usage,
            audio_cache::clear_audio_cache,
            library::loudness_store::get_track_loudness,
            library::loudness_store::clear_loudness_data,
            commands::media::get_video_pages,
            commands::media::get_video_meta,
            commands::media::get_collection_page,
            lyrics::get_cached_video_pages,
            lyrics::clear_video_pages_cache,
            resolve::cancel_prepare_audio,
            #[cfg(debug_assertions)]
            resolve::debug_register_local_stream,
            commands::discovery::search_videos,
            commands::discovery::get_music_ranking,
            commands::runtime::get_stream_source,
            commands::runtime::get_yt_dlp_availability,
            commands::runtime::set_stream_source,
            commands::runtime::open_bilibili_video,
            commands::updates::check_for_update,
            commands::updates::open_update_download,
            choose_background_image,
            load_background_image,
            library::favorites::list_favorites,
            library::collections::toggle_collection_favorite,
            library::collections::add_collection_to_playlist,
            library::collections::add_collection_videos_to_playlist,
            library::collections::update_collection_entry,
            library::collections::remove_collection_entry,
            library::favorites::is_favorite,
            library::favorites::toggle_favorite,
            library::favorites::reorder_favorite,
            library::playlists::list_playlists,
            library::playlists::create_playlist,
            library::playlists::rename_playlist,
            library::playlists::delete_playlist,
            library::playlists::add_to_playlist,
            library::playlists::remove_from_playlist,
            library::playlists::reorder_playlist_item,
            library::playlists::reorder_playlist,
            library::history::record_search_history,
            library::history::get_search_history,
            library::history::clear_search_history,
            library::shortcut_config::get_shortcuts,
            library::shortcut_config::set_shortcuts,
            library::history::record_play,
            library::history::get_play_history,
            library::playback_state::get_playback_state,
            library::playback_state::save_playback_state,
            library::playback_state::clear_playback_state,
            library::unavailable::mark_track_unavailable,
            library::unavailable::clear_track_unavailable,
            library::unavailable::list_unavailable_tracks,
            library::disabled_pages::list_disabled_pages,
            library::disabled_pages::set_page_disabled,
            library::disabled_pages::clear_disabled_pages,
            library::unavailable::purge_unavailable_tracks,
            ai::get_ai_config,
            ai::set_ai_config,
            ai::test_ai_connection,
            ai::get_saved_recommendations,
            lyrics::get_lyrics_by_id,
            lyrics::search_lyrics_songs,
            lyrics::clear_lyrics_cache,
            lyrics::get_lyrics_offset,
            lyrics::set_lyrics_offset,
            commands::media::resolve_lyrics,
            lyrics::get_lyrics_binding,
            lyrics::set_lyrics_binding,
            lyrics::clear_lyrics_binding,
            commands::discovery::get_recommendations,
            library::backup::export_data,
            library::backup::import_data
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}

#[cfg(test)]
pub(crate) mod tests {
    // Test-only Rust lexical scan: blank comments without changing byte offsets or newlines.
    pub(crate) fn without_rust_comments(source: &str) -> String {
        let bytes = source.as_bytes();
        let mut output = bytes.to_vec();
        let mut i = 0;
        while i < bytes.len() {
            let start = i;
            if bytes[i..].starts_with(b"//") {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            } else if bytes[i..].starts_with(b"/*") {
                let mut depth = 1;
                i += 2;
                while i < bytes.len() && depth > 0 {
                    if bytes[i..].starts_with(b"/*") {
                        depth += 1;
                        i += 2;
                    } else if bytes[i..].starts_with(b"*/") {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            } else {
                if bytes[i] == b'r' {
                    let mut quote = i + 1;
                    while bytes.get(quote) == Some(&b'#') {
                        quote += 1;
                    }
                    if bytes.get(quote) == Some(&b'"') {
                        let hashes = quote - i - 1;
                        i = quote + 1;
                        while i < bytes.len() {
                            if bytes[i] == b'"'
                                && bytes.get(i + 1..i + 1 + hashes)
                                    == Some(&bytes[start + 1..quote])
                            {
                                i += 1 + hashes;
                                break;
                            }
                            i += 1;
                        }
                        continue;
                    }
                }
                // An apostrophe starts a char only if followed by an escape or one
                // Unicode scalar and a closing quote; otherwise it is a lifetime.
                let is_char = bytes[i] == b'\''
                    && (bytes.get(i + 1) == Some(&b'\\')
                        || source
                            .get(i + 1..)
                            .and_then(|tail| tail.chars().next())
                            .is_some_and(|c| bytes.get(i + 1 + c.len_utf8()) == Some(&b'\'')));
                if bytes[i] == b'"' || is_char {
                    let quote = bytes[i];
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == b'\\' {
                            i = (i + 2).min(bytes.len());
                        } else if bytes[i] == quote {
                            i += 1;
                            break;
                        } else {
                            i += 1;
                        }
                    }
                } else {
                    i += 1;
                }
                continue;
            }
            for byte in &mut output[start..i] {
                if *byte != b'\n' && *byte != b'\r' {
                    *byte = b' ';
                }
            }
        }
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn comment_scan_preserves_literals_lifetimes_and_newlines() {
        let source = r###"// real comment "keyword"
let a = "https://example.com";
let b = "/* not comment */";
let c = r#"// not comment"#;
let e = r"/* raw */";
let f = r##"// raw with " and #"##;
/* outer /* inner */ outer */
fn f<'a>(x: &'a str) -> char { '"' }
let d = '\'';
let escaped = "\"// still a string";
let unicode = '中'; // 中文 comment
"###;
        let clean = without_rust_comments(source);
        assert_eq!(source.len(), clean.len());
        assert_eq!(
            source
                .match_indices('\n')
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
            clean
                .match_indices('\n')
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        );
        for comment in ["keyword", "outer", "inner", "中文 comment"] {
            assert!(!clean.contains(comment), "{comment}");
        }
        for line in source
            .lines()
            .filter(|line| line.starts_with("let ") || line.starts_with("fn "))
        {
            let code = line.split("; // 中文 comment").next().unwrap();
            assert!(clean.contains(code), "{code}");
        }
        assert_eq!(
            without_rust_comments("a/* x\r\ny */b//z\r\n"),
            "a    \r\n    b   \r\n"
        );
    }

    #[test]
    fn playback_error_keywords_remain_in_rust_sources() {
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../tests/playback-error-contract.json")).unwrap();
        for entry in entries {
            let Some(path) = entry["rustSource"].as_str() else {
                continue;
            };
            let source = match path {
                "src/lib.rs" => include_str!("../../src/lib.rs"),
                "src-tauri/src/guest_playurl.rs" => include_str!("guest_playurl.rs"),
                other => panic!("unexpected Rust source: {other}"),
            };
            let source = without_rust_comments(source);
            let keyword = entry["keyword"].as_str().unwrap();
            if let Some(template) = entry["rustTemplate"].as_str() {
                assert!(
                    source.contains(template),
                    "{path} no longer contains {template:?}"
                );
                let sample = entry["sampleError"].as_str().unwrap();
                let mut segments = template.split("{}");
                let first = segments.next().unwrap();
                let mut rest = sample
                    .strip_prefix(first)
                    .expect("sample must start with template prefix");
                for segment in segments {
                    let position = rest
                        .find(segment)
                        .expect("sample must contain template segments in order");
                    rest = &rest[position + segment.len()..];
                }
                assert!(
                    sample.contains(keyword),
                    "{sample:?} does not contain {keyword:?}"
                );
            } else {
                assert!(
                    source.contains(keyword),
                    "{path} no longer contains {keyword:?}"
                );
            }
        }
    }
}

#[cfg(test)]
mod command_registry;

#[cfg(test)]
mod metadata_consistency;

#[cfg(test)]
mod contract_tests;
