/*
 * 图片主题取色：从用户选的那张背景图算出一套 Palette，喂给三处
 * （主窗强调色 / 小组件 / 任务栏条），让三处跟同一张图同色。
 *
 * 为什么不直接拿全图平均色：
 *   - 一张人物壁纸大面积是皮肤和浅色背景，平均下来是一坨发灰的肉粉；
 *   - 过曝的天空、死黑的暗部会把平均值又拉向两端。
 * 所以规则是：先丢掉过曝和死黑，只在中间调里找**最饱和的那一小撮**当强调色，
 * 底色由这个色相压暗得到（保证深底浅字，文字永远不会糊在图上）。
 * 图本身就没颜色（低饱和）时，退回中性深底 + 图中最亮中间调的色相。
 */
import { CUSTOM_PALETTE_ID, type Palette } from "./palettes.ts";

const SAMPLE_SIZE = 48;
const ALPHA_FLOOR = 128;
const LIGHT_CEILING = 0.9;
const DARK_FLOOR = 0.08;
const MIN_SATURATION = 0.18;
const VIVID_SHARE = 0.2;

type Sample = { h: number; s: number; l: number; r: number; g: number; b: number };

export function rgbToHsl(r: number, g: number, b: number): { h: number; s: number; l: number } {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return { h: 0, s: 0, l };
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h: number;
  if (max === rn) h = ((gn - bn) / d + (gn < bn ? 6 : 0)) / 6;
  else if (max === gn) h = ((bn - rn) / d + 2) / 6;
  else h = ((rn - gn) / d + 4) / 6;
  return { h: h * 360, s, l };
}

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  const sector = (((h % 360) + 360) % 360) / 60;
  const chroma = (1 - Math.abs(2 * l - 1)) * s;
  const x = chroma * (1 - Math.abs((sector % 2) - 1));
  const offset = l - chroma / 2;
  const rgb = sector < 1 ? [chroma, x, 0]
    : sector < 2 ? [x, chroma, 0]
      : sector < 3 ? [0, chroma, x]
        : sector < 4 ? [0, x, chroma]
          : sector < 5 ? [x, 0, chroma] : [chroma, 0, x];
  return rgb.map(value => Math.round((value + offset) * 255)) as [number, number, number];
}

/** 色相是圆形的，跨 0/360 直接平均会算成青色，转成单位向量再平均 */
function meanHue(samples: Sample[]): number {
  let sumX = 0;
  let sumY = 0;
  for (const sample of samples) {
    const radians = (sample.h * Math.PI) / 180;
    sumX += Math.cos(radians);
    sumY += Math.sin(radians);
  }
  const hue = (Math.atan2(sumY, sumX) * 180) / Math.PI;
  return (hue + 360) % 360;
}

function meanOf(samples: Sample[], pick: (sample: Sample) => number): number {
  return samples.reduce((sum, sample) => sum + pick(sample), 0) / Math.max(1, samples.length);
}

function loadImage(dataUrl: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.decoding = "async";
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("背景图解码失败"));
    image.src = dataUrl;
  });
}

function collectSamples(image: HTMLImageElement): Sample[] {
  const canvas = document.createElement("canvas");
  canvas.width = SAMPLE_SIZE;
  canvas.height = SAMPLE_SIZE;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return [];
  context.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
  const pixels = context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data;
  const samples: Sample[] = [];
  for (let i = 0; i < pixels.length; i += 4) {
    const r = pixels[i];
    const g = pixels[i + 1];
    const b = pixels[i + 2];
    if (pixels[i + 3] < ALPHA_FLOOR) continue;
    const { h, s, l } = rgbToHsl(r, g, b);
    // 过曝和死黑不参与统计：天空的高光、场景的暗角都不代表这张图的颜色
    if (l > LIGHT_CEILING || l < DARK_FLOOR) continue;
    samples.push({ h, s, l, r, g, b });
  }
  return samples;
}

/**
 * 中间调里最饱和的一小撮（占 VIVID_SHARE）的平均色。
 * 返回色相、饱和度和「这一撮的平均亮度」——亮度用来决定强调色往亮还是往暗偏。
 */
function vividColor(samples: Sample[]): { h: number; s: number; l: number } | null {
  if (!samples.length) return null;
  const ranked = [...samples].sort((a, b) => b.s - a.s);
  const take = Math.max(1, Math.round(ranked.length * VIVID_SHARE));
  const vivid = ranked.slice(0, take);
  return { h: meanHue(vivid), s: meanOf(vivid, sample => sample.s), l: meanOf(vivid, sample => sample.l) };
}

/** 图里最亮的中间调（低饱和兜底时当强调色用） */
function brightestTone(samples: Sample[]): Sample | null {
  if (!samples.length) return null;
  return samples.reduce((best, sample) => (sample.l > best.l ? sample : best), samples[0]);
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function css(rgb: [number, number, number]): string {
  return `rgb(${rgb[0]}, ${rgb[1]}, ${rgb[2]})`;
}

/**
 * 由色相 + 饱和度造一套配色，图片取色和手动调色共用同一套派生规则，
 * 保证「图里取的」和「手动调的」看起来是同一种东西。
 */
function buildPalette(hue: number, saturation: number, flatImage: boolean, label: string): Palette {
  // 强调色：比原图饱和度略提一点、固定在偏亮的档位，深底上才看得见
  const accent = hslToRgb(hue, clamp(saturation * 1.15, 0.3, 0.85), flatImage ? 0.72 : 0.64);
  // 条底：同一色相压暗，做成一段很窄的渐变；纯色近似取中点，喂给 GDI
  const bgTop = hslToRgb(hue, saturation * 0.55, 0.2);
  const bgBottom = hslToRgb(hue, saturation * 0.5, 0.11);
  const bgMid = hslToRgb(hue, saturation * 0.52, 0.16);
  const text = hslToRgb(hue, saturation * 0.2, 0.95);
  const textMid = hslToRgb(hue, saturation * 0.16, 0.93);

  return {
    id: CUSTOM_PALETTE_ID,
    label,
    accent,
    skin: "ink",
    barBg: `linear-gradient(135deg, ${css(bgTop)}, ${css(bgBottom)})`,
    barText: `rgb(${text[0]}, ${text[1]}, ${text[2]})`,
    barBgRgb: bgMid,
    barTextRgb: textMid,
    group: "gradient",
  };
}

/** 手动调色相/饱和度用的入口；饱和度过低时按「没颜色」处理，避免一身灰 */
export function paletteFromHue(hue: number, saturation: number, label = "自定义配色"): Palette {
  const normalizedHue = ((hue % 360) + 360) % 360;
  const clamped = clamp(saturation, 0, 0.9);
  return buildPalette(normalizedHue, clamped, clamped < MIN_SATURATION, label);
}


/**
 * 从背景图派生一套配色。
 * 失败（图读不出来 / 画布拿不到像素）时返回 null，调用方保持原配色即可。
 */
export async function paletteFromImage(dataUrl: string): Promise<Palette | null> {
  if (!dataUrl) return null;
  let image: HTMLImageElement;
  try {
    image = await loadImage(dataUrl);
  } catch {
    return null;
  }
  const samples = collectSamples(image);
  if (!samples.length) return null;

  const vivid = vividColor(samples);
  // 图本身没颜色（灰阶图 / 极淡的水墨）：不硬造色相，用中性深底 + 最亮中间调
  const strong = vivid && vivid.s >= MIN_SATURATION ? vivid : null;
  const flatImage = strong === null;
  const tone = brightestTone(samples);
  const hue = strong ? strong.h : tone ? rgbToHsl(tone.r, tone.g, tone.b).h : 0;
  const saturation = strong ? clamp(strong.s, MIN_SATURATION, 0.9) : 0.05;

  return buildPalette(hue, saturation, flatImage, "图片取色");
}
