/*
 * 本地存储门面 —— 所有 localStorage 直读直写都从这里走。
 *
 * 为什么集中成一处：
 *   1. key 原先散在 14 个文件里，有几个还被写成字符串字面量（"mini-palette"），
 *      想改名得全局搜字符串，漏一处就是静默失效，不报错也测不出来；
 *   2. 一部分调用点没包 try/catch —— 隐私模式、配额满、受限 WebView 里
 *      localStorage 本身会抛，那几处会直接把异常抛到 UI 线程外面。
 *
 * key 的**值**保持原样（bilibili-music.* / mini-*）：老版本用户的数据还躺在里面，
 * 改值等于把他们的设置清空。
 */

// ── key 清单（唯一来源）────────────────────────────────────────────
/** 主窗口明暗：dark / light（主题拆成「明暗 + 背景」两轴后，dynamic / image 归到 BACKGROUND_MODE_KEY） */
export const THEME_KEY = "bilibili-music.theme";
/** 背景轴：solid / image / cover。老配置里的 theme=dynamic|image 由 setTheme 迁移到这里 */
export const BACKGROUND_MODE_KEY = "bilibili-music.background-mode";
/** 主窗口强调色 id */
export const ACCENT_COLOR_KEY = "bilibili-music.accent-color";
/** 用户音量 0~1 */
export const VOLUME_KEY = "bilibili-music.volume";
/** 响度均衡开关，存 "true"/"false" */
export const LOUDNESS_KEY = "bilibili-music.loudness-normalization";
/** 侧栏宽度 + 折叠状态，JSON */
export const SIDEBAR_KEY = "bilibili-music.sidebar";
/** 曲目列表各列宽度，JSON */
export const TRACK_COLUMNS_KEY = "bilibili-music.track-columns";
/** 上次搜索词 */
export const LAST_SEARCH_KEY = "bilibili-music.last-search";
/** 合集随机顺序偏好 */
export const SHUFFLE_ORDER_KEY = "bilibili-music.shuffle-collection-order";
/** 合集随机数量上限 */
export const SHUFFLE_LIMIT_KEY = "bilibili-music.shuffle-collection-limit";

/** 当前配色 id（可能是 40 套之一的 id，也可能是 "custom"） */
export const PALETTE_KEY = "mini-palette";
/** 图片主题算出来的那套自定义配色，JSON */
export const CUSTOM_PALETTE_KEY = "mini-custom-palette";
/** 跟随封面取色开关，存 "1"/"0" */
export const FOLLOW_SONG_KEY = "mini-follow-song";
/** 启动时恢复哪种显示方式：bar / mini / off */
export const SURFACE_MODE_KEY = "mini-surface-mode";
/** 迷你窗旧版 data-skin（历史字段，新版式按 palette 走，只为兼容保留） */
export const SKIN_KEY = "mini-skin";
/** 迷你窗缩放倍率 */
export const MINI_SCALE_KEY = "mini-scale";
/** 迷你窗上次停的位置，JSON {x, y} */
export const MINI_POSITION_KEY = "bilibili-music.mini-player-position";

// ── 读写原语（一律不抛）──────────────────────────────────────────
/** 读字符串；localStorage 不可用时返回 null */
export function readString(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** 写字符串；失败（配额满 / 不可用）返回 false，不抛 */
export function writeString(key: string, value: string): boolean {
  try {
    localStorage.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

/** 删一个 key；失败静默 */
export function removeKey(key: string): void {
  try {
    localStorage.removeItem(key);
  } catch {
    /* 受限环境里删不掉也只能算了，下次读到旧值不算致命 */
  }
}

/** 读 JSON；没值或解析失败都返回 null（调用方自己判形状） */
export function readJson<T>(key: string): T | null {
  const raw = readString(key);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

/** 写 JSON；失败静默返回 false */
export function writeJson(key: string, value: unknown): boolean {
  return writeString(key, JSON.stringify(value));
}
