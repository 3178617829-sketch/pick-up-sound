/*
 * 跟随歌曲：每首歌对应一套配色，三处（主界面 / 小组件 / 任务栏条）共用。
 *
 * 取色链路：主窗 dynamic 主题已经把封面主色算进 .dynamic-background-layer 的
 * --dynamic-color-1，这里直接读它，再在 40 套 palette 里挑「色相最接近 + 明暗最接近」
 * 的一套。不再拿封面文件名做哈希——那样跟颜色毫无关系。
 *
 * 三种状态：
 *   1. mini-follow-song = "1"  → 跟随歌曲（本文件负责）
 *   2. 用户手选某套配色        → 关掉跟随（SettingsPanel 负责），本文件不再干预
 *   3. 读不到封面主色（封面还没加载 / 纯灰封面）→ 保持现状，不瞎抽一套
 */
import { CUSTOM_PALETTE_ID, PALETTES, activePaletteId, readCustomPalette, solidOf, type Palette } from "./palettes.ts";
import { FOLLOW_SONG_KEY, readString } from "./storage.ts";

let lastSrc = "";
let running = false;

/**
 * 用户自己调的那套颜色：只要它被选中，三处就都跟着它，
 * 跟封面、跟背景图都没关系——手选了就是手选了。
 * 但「跟随歌曲」是更明确的意图：开关打开就说明要跟封面走，此时让位。
 */
function customPaletteActive(): Palette | null {
  if (following()) return null;
  if (activePaletteId() !== CUSTOM_PALETTE_ID) return null;
  return readCustomPalette();
}

/** 同一套颜色的指纹：换图后 id 还是 custom，只能靠内容判断变没变 */
function signature(palette: Palette): string {
  return `${palette.accent.join(",")}|${palette.barBgRgb.join(",")}`;
}

function parseColor(text: string): [number, number, number] | null {
  const rgb = text.match(/rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)/i);
  if (rgb) return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])];
  const hex = text.match(/#([0-9a-f]{6})\b/i);
  if (hex) {
    const n = parseInt(hex[1], 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  }
  return null;
}

/** rgb → [h(0-360), s(0-1), l(0-1)] */
function hslOf([r, g, b]: [number, number, number]): [number, number, number] {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return [0, 0, l];
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h: number;
  if (max === rn) h = ((gn - bn) / d + (gn < bn ? 6 : 0)) / 6;
  else if (max === gn) h = ((bn - rn) / d + 2) / 6;
  else h = ((rn - gn) / d + 4) / 6;
  return [h * 360, s, l];
}

/**
 * 主窗 dynamic 背景当前正在显示的那层，第一个颜色就是封面主色。
 * 直接读 DOM 上的自定义属性，不 import features 层，避免 shared → features 的依赖倒挂。
 */
function readCoverColor(): [number, number, number] | null {
  const layer = document.querySelector(".dynamic-background-layer.is-visible") as HTMLElement | null;
  if (!layer) return null;
  const raw = layer.style.getPropertyValue("--dynamic-color-1").trim()
    || getComputedStyle(layer).getPropertyValue("--dynamic-color-1").trim();
  if (!raw) return null;
  return parseColor(raw);
}

/**
 * 在 40 套里挑底色和封面主色最接近的一套。
 * 打分：色相距离（按封面饱和度加权）+ 明度差 + 饱和度差。
 * 用 solidOf 把渐变取成代表色，否则渐变套的 barBg 解析不出 RGB。
 */
function pickNearestPalette(cover: [number, number, number]): Palette {
  const [h1, s1, l1] = hslOf(cover);
  let best = PALETTES[0];
  let bestScore = Number.POSITIVE_INFINITY;
  for (const p of PALETTES) {
    const rgb = parseColor(solidOf(p.barBg));
    if (!rgb) continue;
    const [h2, s2, l2] = hslOf(rgb);
    let dh = Math.abs(h1 - h2);
    if (dh > 180) dh = 360 - dh;
    // 近灰的封面没有可用色相，就别让色相主导选择，交给明暗决定
    const hueScore = s1 < 0.16 ? 0 : dh * (1 + s1 * 1.6);
    const score = hueScore + Math.abs(l1 - l2) * 70 + Math.abs(s1 - s2) * 40;
    if (score < bestScore) {
      bestScore = score;
      best = p;
    }
  }
  return best;
}

/**
 * 只认「当前正在播放」的封面：底部播放条那张对所有页面都成立，
 * 唱片中心那张只有首页才有，所以优先用底部播放条。
 */
function findCover(): HTMLImageElement | null {
  const candidates = [
    "footer.player-bar .player-cover-button img",
    "footer.player-bar img",
    ".vinyl-label img",
    ".now-playing-cover img",
  ];
  for (const sel of candidates) {
    const el = document.querySelector(sel) as HTMLImageElement | null;
    if (el && el.src && !el.src.startsWith("data:")) return el;
  }
  return null;
}

/** 当前封面主色对应的 palette（拿不到封面主色时返回 null，绝不瞎猜） */
export function paletteForCurrentCover(): Palette | null {
  const custom = customPaletteActive();
  if (custom) return custom;
  const cover = readCoverColor();
  if (!cover) return null;
  return pickNearestPalette(cover);
}

function following(): boolean {
  return readString(FOLLOW_SONG_KEY) === "1";
}

export function startFollowSong(apply: (p: Palette) => void): void {
  if (running) return;
  running = true;
  /** 手选的自定义配色：跟随歌曲打开时让位，否则它说了算 */
  function applyCustomPalette(): boolean {
    const custom = customPaletteActive();
    if (!custom) return false;
    const key = `custom:${signature(custom)}`;
    if (key === lastSrc) return true;
    lastSrc = key;
    apply(custom);
    return true;
  }
  /**
   * 按当前封面同步一次（跟随歌曲的常规动作）。
   *
   * 记账时机是这条链路的关键：封面主色由动态背景层异步算出来，切歌那一瞬间
   * 通常还没就绪。早期版本先记 `lastSrc` 再去读色，读不到就 return——这首歌
   * 于是被记成「已处理」，之后每 2 秒轮询都因为 `key === lastSrc` 直接跳过，
   * 表现就是「跟随歌曲时灵时不灵，全看切歌那一下主色算完没有」。
   * 现在只有真正拿到配色并应用之后才记账；读不到就等下一轮（2 秒一次，很便宜）。
   * key 里带上 palette.id：同一首歌如果取色结果换了套，也允许再应用一次。
   */
  function syncFromCover(): void {
    if (customPaletteActive()) return; // 自定义配色优先，记账交给 applyCustomPalette
    const img = findCover();
    if (!img) return;
    const palette = paletteForCurrentCover();
    if (!palette) return; // 主色还没算出来，下一轮再试，别把这首歌记成已处理
    const key = `${img.src.split("/").pop() ?? img.src}|${palette.id}`;
    if (key === lastSrc) return;
    lastSrc = key;
    apply(palette);
  }
  window.addEventListener("follow-song-change", () => {
    lastSrc = "";
    // 手选了配色 / 刚打开跟随歌曲，都要立刻见效，不能等下一轮轮询
    if (applyCustomPalette()) return;
    if (following()) syncFromCover();
  });
  // 切换主题（尤其切进 / 切出图片）时要立刻重算，不能等下一首歌
  new MutationObserver(() => {
    lastSrc = "";
    if (!applyCustomPalette() && following()) syncFromCover();
  }).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

  // 启动即同步：dynamic 背景要等封面加载、算完色才有 --dynamic-color-1，
  // 所以每 500ms 试一次，拿到就收工；最多试 30 次（15 秒）后放弃，
  // 放弃时保持用户上次的配色不动（这就是「每次开软件颜色都不一样」的根治点）。
  let bootTries = 0;
  const bootTimer = window.setInterval(() => {
    if (!following()) {
      window.clearInterval(bootTimer);
      return;
    }
    if (applyCustomPalette()) {
      window.clearInterval(bootTimer);
      return;
    }
    const palette = paletteForCurrentCover();
    if (palette) {
      const img = findCover();
      if (img) lastSrc = `${img.src.split("/").pop() ?? img.src}|${palette.id}`;
      apply(palette);
      window.clearInterval(bootTimer);
      return;
    }
    if (++bootTries >= 30) window.clearInterval(bootTimer);
  }, 500);

  window.setInterval(() => {
    if (!following()) return;
    syncFromCover();
  }, 2000);
}
