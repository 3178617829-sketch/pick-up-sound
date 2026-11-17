/*
 * 统一配色（40 套）— 主窗口强调色、迷你窗、任务栏条，三处共用同一份定义。
 * 换色只改这里；界面从 localStorage 的 mini-palette 读取 id。
 *
 * 组成：12 套纯色 + 28 套渐变。
 * 其中 barBgRgb / barTextRgb 是给**原生 GDI 任务栏条**用的纯色版本：
 * GDI 解析不了 rgba / linear-gradient，渐变取两端的中点近似。
 *
 * 三个派生量的规则（三处共用，别各自实现）：
 *   1. accent  —— 强调色，按底色亮度**双向校正**：深底提亮、浅底压深（fixAccent）
 *   2. barBg   —— 卡片/条底色，可能是 rgba 也可能渐变，直接喂给 CSS
 *   3. barText —— 文字色，浅底深字、深底浅字
 */
import { CUSTOM_PALETTE_KEY, FOLLOW_SONG_KEY, PALETTE_KEY, SKIN_KEY, SURFACE_MODE_KEY, readString, removeKey, writeString } from "./storage.ts";
import { setRootVar } from "./root-style.ts";

export type Palette = {
  id: string;
  label: string;
  /** 主窗口强调色 + 任务栏条强调色 [r, g, b] */
  accent: [number, number, number];
  /** 迷你窗 data-skin 值（历史字段，新版式按 palette 走） */
  skin: string;
  /** 任务栏条 / 小组件底色（网页版，可以是渐变） */
  barBg: string;
  /** 任务栏条 / 小组件文字色 */
  barText: string;
  /** 任务栏条底色（原生 GDI 用的纯色近似，渐变取中点） */
  barBgRgb: [number, number, number];
  /** 任务栏条文字色（原生 GDI 用） */
  barTextRgb: [number, number, number];
  /** 分组：solid = 纯色系，gradient = 渐变色系 */
  group: "solid" | "gradient";
};

export const PALETTES: Palette[] = [
  // ── 纯色系 12 套 ──
  { id: "wood",    label: "墨褐",     accent: [196, 164, 120], skin: "wood",   barBg: "rgb(28, 26, 24)",   barText: "#c9c2b6", barBgRgb: [28, 26, 24],    barTextRgb: [201, 194, 182], group: "solid" },
  { id: "teal",    label: "青绿",     accent: [45, 138, 128],  skin: "teal",   barBg: "rgba(16, 42, 40, 0.88)",   barText: "#eef6f3", barBgRgb: [16, 42, 40],    barTextRgb: [238, 246, 243], group: "solid" },
  { id: "ink",     label: "纯黑",     accent: [150, 150, 160], skin: "ink",    barBg: "rgba(14, 14, 16, 0.90)",   barText: "#f0f0f2", barBgRgb: [14, 14, 16],    barTextRgb: [240, 240, 242], group: "solid" },
  { id: "indigo",  label: "深夜蓝",   accent: [92, 108, 196],  skin: "indigo", barBg: "rgba(20, 22, 44, 0.88)",   barText: "#eef0fa", barBgRgb: [20, 22, 44],    barTextRgb: [238, 240, 250], group: "solid" },
  { id: "sunset",  label: "暖霞",     accent: [214, 122, 72],  skin: "sunset", barBg: "rgba(44, 26, 18, 0.88)",   barText: "#faeee6", barBgRgb: [44, 26, 18],    barTextRgb: [250, 238, 230], group: "solid" },
  { id: "paper",   label: "纯白",     accent: [138, 106, 59],  skin: "paper",  barBg: "rgb(255, 255, 255)", barText: "#2a241c", barBgRgb: [255, 255, 255], barTextRgb: [42, 36, 28],   group: "solid" },
  { id: "blue",    label: "蓝",       accent: [122, 166, 231], skin: "indigo", barBg: "rgba(22, 30, 48, 0.88)",   barText: "#eef3fa", barBgRgb: [22, 30, 48],    barTextRgb: [238, 243, 250], group: "solid" },
  { id: "purple",  label: "紫",       accent: [189, 145, 236], skin: "aurora", barBg: "rgba(34, 24, 48, 0.88)",   barText: "#f4eefa", barBgRgb: [34, 24, 48],    barTextRgb: [244, 238, 250], group: "solid" },
  { id: "green",   label: "绿",       accent: [87, 185, 65],   skin: "teal",   barBg: "rgba(20, 36, 18, 0.88)",   barText: "#eef6ec", barBgRgb: [20, 36, 18],    barTextRgb: [238, 246, 236], group: "solid" },
  { id: "cyan",    label: "青",       accent: [73, 178, 191],  skin: "deep",   barBg: "rgba(16, 34, 38, 0.88)",   barText: "#e9f7f8", barBgRgb: [16, 34, 38],    barTextRgb: [233, 247, 248], group: "solid" },
  { id: "creme",   label: "奶油",     accent: [176, 132, 74],  skin: "paper",  barBg: "rgba(240, 232, 214, 0.94)", barText: "#2e2418", barBgRgb: [240, 232, 214], barTextRgb: [46, 36, 24],   group: "solid" },
  { id: "mint",    label: "薄荷",     accent: [72, 150, 124],  skin: "teal",   barBg: "rgba(224, 236, 228, 0.94)", barText: "#1f3028", barBgRgb: [224, 236, 228], barTextRgb: [31, 48, 40],   group: "solid" },

  // ── 渐变色系 28 套（旧 8 套 + 新 20 套）──
  { id: "dawn",      label: "晚霞渐变",   accent: [228, 132, 110], skin: "dawn",   barBg: "linear-gradient(135deg, #f08a5d, #9e2f52)",   barText: "#fdf0ea", barBgRgb: [199, 93, 88],   barTextRgb: [253, 240, 234], group: "gradient" },
  { id: "deep",      label: "深海渐变",   accent: [64, 138, 158],  skin: "deep",   barBg: "linear-gradient(135deg, #2e4a7d, #14524f)",     barText: "#e8f4f8", barBgRgb: [33, 78, 102],   barTextRgb: [232, 244, 248], group: "gradient" },
  { id: "aurora",    label: "极光渐变",   accent: [120, 190, 180], skin: "aurora", barBg: "linear-gradient(135deg, #17896f, #5b3d8c)",   barText: "#e9f8f4", barBgRgb: [57, 99, 126],   barTextRgb: [233, 248, 244], group: "gradient" },
  { id: "mist",      label: "雾灰渐变",   accent: [160, 170, 186], skin: "mist",   barBg: "linear-gradient(135deg, #c98a8f, #5e6b85)",  barText: "#f5f7fa", barBgRgb: [148, 123, 138], barTextRgb: [245, 247, 250], group: "gradient" },
  { id: "rose",      label: "玫红渐变",   accent: [220, 120, 140], skin: "dawn",   barBg: "linear-gradient(135deg, #cb5e7c, #7a2c50)",   barText: "#fceef2", barBgRgb: [163, 69, 102],  barTextRgb: [252, 238, 242], group: "gradient" },
  { id: "slate",     label: "石墨渐变",   accent: [140, 146, 160], skin: "mist",   barBg: "linear-gradient(135deg, #484e5c, #1a1c22)",      barText: "#eef0f4", barBgRgb: [49, 53, 63],    barTextRgb: [238, 240, 244], group: "gradient" },
  { id: "burgundy",  label: "酒红渐变",   accent: [206, 158, 96],  skin: "dawn",   barBg: "linear-gradient(135deg, #6e1c30, #340e1a)",     barText: "#f7e9e4", barBgRgb: [81, 21, 37],    barTextRgb: [247, 233, 228], group: "gradient" },
  { id: "navy",      label: "藏青渐变",   accent: [214, 178, 98],  skin: "indigo", barBg: "linear-gradient(135deg, #1c2c4a, #0e1626)",      barText: "#e8eef8", barBgRgb: [21, 33, 56],    barTextRgb: [232, 238, 248], group: "gradient" },

  { id: "sunrise",   label: "晨雾渐变",   accent: [201, 96, 58],   skin: "dawn",   barBg: "linear-gradient(135deg, #fbe3c4, #f2c9a0 45%, #e8a0a8)",             barText: "#3a2418", barBgRgb: [242, 201, 160], barTextRgb: [58, 36, 24],    group: "gradient" },
  { id: "dune",      label: "沙丘渐变",   accent: [224, 176, 112], skin: "wood",   barBg: "linear-gradient(135deg, #c9a877, #8a6a42 55%, #3a2814)",              barText: "#f6efe6", barBgRgb: [138, 106, 66],  barTextRgb: [246, 239, 230], group: "gradient" },
  { id: "pine",      label: "松林渐变",   accent: [79, 174, 122],  skin: "teal",   barBg: "linear-gradient(135deg, #1a3a2e, #102620 60%, #0a1410)",              barText: "#e4f0e8", barBgRgb: [16, 38, 32],    barTextRgb: [228, 240, 232], group: "gradient" },
  { id: "glacier",   label: "冰川渐变",   accent: [58, 134, 168],  skin: "mist",   barBg: "linear-gradient(135deg, #eef7fc, #cfe4ef 50%, #a8cbdd)",              barText: "#1d3846", barBgRgb: [207, 228, 239], barTextRgb: [29, 56, 70],    group: "gradient" },
  { id: "violet",    label: "紫罗兰渐变", accent: [168, 120, 216], skin: "aurora", barBg: "linear-gradient(135deg, #4a2a6e, #311a4a 55%, #1a0e26)",              barText: "#f0e6fa", barBgRgb: [49, 26, 74],    barTextRgb: [240, 230, 250], group: "gradient" },
  { id: "caramel",   label: "焦糖渐变",   accent: [216, 154, 85],  skin: "wood",   barBg: "linear-gradient(135deg, #b87a42, #7a4a22 55%, #2e1a0a)",              barText: "#fbead6", barBgRgb: [122, 74, 34],   barTextRgb: [251, 234, 214], group: "gradient" },
  { id: "sakura",    label: "樱渐变",     accent: [210, 96, 127],  skin: "dawn",   barBg: "linear-gradient(135deg, #fdf0f4, #f8dde5 50%, #f2bccb)",              barText: "#3d1f28", barBgRgb: [248, 221, 229], barTextRgb: [61, 31, 40],    group: "gradient" },
  { id: "lava",      label: "熔岩渐变",   accent: [255, 138, 69],  skin: "sunset", barBg: "linear-gradient(135deg, #e05a20, #8a2a0a 55%, #200804)",              barText: "#ffe6d6", barBgRgb: [138, 42, 10],   barTextRgb: [255, 230, 214], group: "gradient" },
  { id: "moss",      label: "苔渐变",     accent: [157, 184, 90],  skin: "teal",   barBg: "linear-gradient(135deg, #6a7a38, #3e4a20 55%, #161c0a)",              barText: "#eef2dd", barBgRgb: [62, 74, 32],    barTextRgb: [238, 242, 221], group: "gradient" },
  { id: "dusk",      label: "黄昏渐变",   accent: [232, 138, 90],  skin: "dawn",   barBg: "linear-gradient(135deg, #3a1f4a, #7a3a58 50%, #c06a48)",              barText: "#fbe8e0", barBgRgb: [122, 58, 88],   barTextRgb: [251, 232, 224], group: "gradient" },
  { id: "sea",       label: "海盐渐变",   accent: [63, 143, 174],  skin: "mist",   barBg: "linear-gradient(135deg, #f2fafc, #cfe7f0 50%, #b0d4e2)",              barText: "#1a3440", barBgRgb: [207, 231, 240], barTextRgb: [26, 52, 64],    group: "gradient" },
  { id: "copper",    label: "铜渐变",     accent: [196, 122, 69],  skin: "wood",   barBg: "linear-gradient(135deg, #9a5a34, #5e3218 55%, #241008)",              barText: "#f6e8da", barBgRgb: [94, 50, 24],    barTextRgb: [246, 232, 218], group: "gradient" },
  { id: "sumi",      label: "墨渐变",     accent: [122, 128, 144], skin: "ink",    barBg: "linear-gradient(135deg, #34343a, #1e1e22 55%, #0c0c0e)",              barText: "#eceef0", barBgRgb: [30, 30, 34],    barTextRgb: [236, 238, 240], group: "gradient" },
  { id: "peach",     label: "桃渐变",     accent: [210, 101, 74],  skin: "dawn",   barBg: "linear-gradient(135deg, #fde4d6, #f7c4b0 50%, #efa088)",              barText: "#40211a", barBgRgb: [247, 196, 176], barTextRgb: [64, 33, 26],    group: "gradient" },
  { id: "star",      label: "星空渐变",   accent: [106, 122, 216], skin: "indigo", barBg: "linear-gradient(135deg, #22306e, #131a44 55%, #05060e)",              barText: "#e4e8fa", barBgRgb: [19, 26, 68],    barTextRgb: [228, 232, 250], group: "gradient" },
  { id: "honey",     label: "蜂蜜渐变",   accent: [214, 140, 30],  skin: "wood",   barBg: "linear-gradient(135deg, #f0c25a, #c08a18 55%, #6a4004)",              barText: "#2a1a04", barBgRgb: [192, 138, 24],  barTextRgb: [42, 26, 4],     group: "gradient" },
  { id: "turquoise", label: "松石渐变",   accent: [58, 208, 192],  skin: "teal",   barBg: "linear-gradient(135deg, #22b0a2, #128a7e 55%, #0a3a38)",              barText: "#e0f6f4", barBgRgb: [18, 138, 126],  barTextRgb: [224, 246, 244], group: "gradient" },
  { id: "rust",      label: "铁锈渐变",   accent: [216, 112, 72],  skin: "sunset", barBg: "linear-gradient(135deg, #a8502c, #6a2a14 55%, #2a0e08)",              barText: "#f8e4d8", barBgRgb: [106, 42, 20],   barTextRgb: [248, 228, 216], group: "gradient" },
  { id: "lavender",  label: "薰衣草渐变", accent: [138, 111, 192], skin: "aurora", barBg: "linear-gradient(135deg, #f2ecfa, #e0d3f0 50%, #c2b2db)",              barText: "#2e2240", barBgRgb: [224, 211, 240], barTextRgb: [46, 34, 64],    group: "gradient" },
  { id: "midnight",  label: "午夜渐变",   accent: [74, 106, 200],  skin: "indigo", barBg: "linear-gradient(135deg, #12204a, #0a1028 55%, #04050c)",              barText: "#dce4fa", barBgRgb: [10, 16, 40],    barTextRgb: [220, 228, 250], group: "gradient" },
];

// key 的定义搬到了 shared/storage.ts（唯一来源），这里转出，老的 import 路径不用改。
export { CUSTOM_PALETTE_KEY, FOLLOW_SONG_KEY, PALETTE_KEY, SKIN_KEY, SURFACE_MODE_KEY };
/** 图片主题下「从背景图算出来的那套颜色」，一次一张，跟着图换 */
export const CUSTOM_PALETTE_ID = "custom";
export const DEFAULT_PALETTE = "wood";

/** 读回图片主题那套自定义配色；没有或数据坏了返回 null */
export function readCustomPalette(): Palette | null {
  try {
    const raw = readString(CUSTOM_PALETTE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<Palette> | null;
    if (!parsed || !Array.isArray(parsed.accent) || !Array.isArray(parsed.barBgRgb)) return null;
    return parsed as Palette;
  } catch {
    return null;
  }
}

export function saveCustomPalette(palette: Palette | null): void {
  if (!palette) {
    removeKey(CUSTOM_PALETTE_KEY);
    return;
  }
  writeString(CUSTOM_PALETTE_KEY, JSON.stringify(palette));
}

/** 让跟随链立刻重算。settings 域在无 DOM 的 Node 环境里也要能跑，所以派发前先探一下。 */
function notifyFollowSongChange(): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent("follow-song-change"));
}

/**
 * 图片主题生效：把这套从图里算出来的颜色设为当前配色，并让跟随链立刻重算。
 * 只动颜色，不碰图片本身。
 */
export function activateCustomPalette(palette: Palette): void {
  saveCustomPalette(palette);
  writeString(PALETTE_KEY, CUSTOM_PALETTE_ID);
  notifyFollowSongChange();
}

/** 退出图片主题：清掉这套颜色，配色回到跟随歌曲那条路 */
export function deactivateCustomPalette(): void {
  saveCustomPalette(null);
  if (readString(PALETTE_KEY) === CUSTOM_PALETTE_ID) {
    removeKey(PALETTE_KEY);
  }
  notifyFollowSongChange();
}

export function paletteById(id: string | null | undefined): Palette {
  if (id === CUSTOM_PALETTE_ID) {
    const custom = readCustomPalette();
    if (custom) return custom;
  }
  // 未知 id 回落到 DEFAULT_PALETTE；末尾的 PALETTES[0] 只防 DEFAULT_PALETTE 拼错。
  return PALETTES.find(p => p.id === id)
    ?? PALETTES.find(p => p.id === DEFAULT_PALETTE)
    ?? PALETTES[0];
}

/** 当前生效的配色 id（没设过就是 null，调用方自行兜底） */
export function activePaletteId(): string | null {
  return readString(PALETTE_KEY);
}

// ───────────────────────── 派生量（三处共用） ─────────────────────────

function toHsl(rgb: [number, number, number]): { h: number; s: number; l: number } {
  const r = rgb[0] / 255, g = rgb[1] / 255, b = rgb[2] / 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b), l = (max + min) / 2, d = max - min;
  let h = 0, s = 0;
  if (d) {
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    h = max === r ? ((g - b) / d + (g < b ? 6 : 0)) : max === g ? ((b - r) / d + 2) : ((r - g) / d + 4);
    h /= 6;
  }
  return { h, s, l };
}

function fromHsl(h: number, s: number, l: number): [number, number, number] {
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s, p = 2 * l - q;
  const f = (t: number) => { t = (t + 1) % 1; return t < 1 / 6 ? p + (q - p) * 6 * t : t < 1 / 2 ? q : t < 2 / 3 ? p + (q - p) * (2 / 3 - t) * 6 : p; };
  return [f(h + 1 / 3), f(h), f(h - 1 / 3)].map(v => Math.round(v * 255)) as [number, number, number];
}

/** 底色亮度 0~1（用 GDI 近似色算，够用） */
export function paletteLuma(p: Palette): number {
  const [r, g, b] = p.barBgRgb;
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

/** 浅底配色（要用深字） */
export function isLightPalette(p: Palette): boolean {
  return paletteLuma(p) > 0.62;
}

/**
 * 强调色的双向校正：深底提亮、浅底压深，保证发光和主键 siempre 看得见。
 * 返回校正后的 [r,g,b]；本来就合适就原样返回。
 */
export function fixAccent(p: Palette): [number, number, number] {
  const luma = paletteLuma(p);
  const { h, s, l } = toHsl(p.accent);
  if (luma < 0.45 && l < 0.58) return fromHsl(h, Math.min(1, s + 0.10), 0.70);
  if (luma > 0.62 && l > 0.46) return fromHsl(h, Math.min(1, s + 0.06), 0.44);
  return p.accent;
}

/** 主键（实心圆按钮）上的文字/图标色 */
export function onAccent(p: Palette): string {
  const [r, g, b] = fixAccent(p);
  const l = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  return l > 0.62 ? "#231c17" : "#ffffff";
}

/** 把 [r,g,b] 变成 CSS 用的 "r, g, b" */
export function rgbString(rgb: [number, number, number]): string {
  return `${rgb[0]}, ${rgb[1]}, ${rgb[2]}`;
}

/** 渐变背景里取第一段颜色当纯色兜底（主窗的 --root 必须是颜色，不能是 gradient） */
export function solidOf(bg: string): string {
  if (!bg || !bg.includes("gradient")) return bg;
  const hex = bg.match(/#[0-9a-fA-F]{3,8}/);
  return hex ? hex[0] : "#241a12";
}

/** 当前配色的全部派生量，CSS 变量一次给齐 */
export function paletteVars(p: Palette): Record<string, string> {
  const accent = fixAccent(p);
  return {
    "--pal-accent": `rgb(${accent[0]}, ${accent[1]}, ${accent[2]})`,
    "--pal-accent-rgb": `${accent[0]}, ${accent[1]}, ${accent[2]}`,
    "--pal-bg": p.barBg,
    "--pal-bg-solid": solidOf(p.barBg),
    "--pal-fg": p.barText,
    "--pal-on": onAccent(p),
    "--pal-luma": String(Math.round(paletteLuma(p) * 1000) / 1000),
  };
}

/**
 * 把一套配色真正应用到界面：强调色三分量 + 全套 --pal-* 变量 + 主窗底色与文字。
 * 手选配色和「跟随歌曲」都走这一条路 —— 只写 --accent-r/g/b，--accent 与 --accent-rgb
 * 由 styles.css 的 var() 链自己派生；手写覆盖它们会让两条通道各写一半。
 *
 * 底色与文字直接取配色自己的 barBg / barText：三处（主窗 / 小组件 / 任务栏条）
 * 用同一份定义，选一套配色才是真的「三处一起变」。
 * image / dynamic 模式下底色由背景图与封面决定，这里只推强调色，不去抢。
 */
export function applyPaletteToRoot(p: Palette, surface: "dark" | "light" | "image" | "dynamic" = "dark"): void {
  const accent = fixAccent(p);
  setRootVar("--accent-r", String(accent[0]));
  setRootVar("--accent-g", String(accent[1]));
  setRootVar("--accent-b", String(accent[2]));
  for (const [name, value] of Object.entries(paletteVars(p))) setRootVar(name, value);
  if (surface === "image" || surface === "dynamic") return;
  setRootVar("--root", solidOf(p.barBg));
  // 设置面板/弹窗这些浮动件用的是 --floating-surface（--surface），不跟着配色走就会
  // 出现「深底配色 + 浅色面板 + 浅色文字」的白底白字，所以这里一并接管。
  setRootVar("--floating-surface", solidOf(p.barBg));
  setRootVar("--text", p.barText);
  setRootVar("--text2", `color-mix(in srgb, ${p.barText} 76%, transparent)`);
  setRootVar("--text3", `color-mix(in srgb, ${p.barText} 54%, transparent)`);
  setRootVar("--line", `color-mix(in srgb, ${p.barText} 13%, transparent)`);
  // 播放键有两处：中间大圆钮吃 --accent，底部走 --play-bg / --play-fg。
  // 不一起写就会出现「中间橙、底部金」的两个强调色，这里统一成同一个。
  setRootVar("--play-bg", `rgb(${rgbString(accent)})`);
  setRootVar("--play-fg", onAccent(p));
}
