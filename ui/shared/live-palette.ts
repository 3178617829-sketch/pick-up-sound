/*
 * 当前生效的配色 —— 主窗、小组件、任务栏条三处唯一的色源。
 *
 * 为什么要这一层：主窗的强调色来自 CSS 变量，任务栏条走 paletteById(存储里的 id)，
 * 小组件走 store 里的 accentColor（只有六个 key）。三条路各算一遍，就会出现
 * 「主窗变金、条变金、小组件还是粉」这种不统一。现在一律从这里取。
 */
import { DEFAULT_PALETTE, paletteById, type Palette } from "./palettes.ts";

let current: Palette = paletteById(DEFAULT_PALETTE);
const listeners = new Set<() => void>();

/** 当前真正在用的配色（跟随封面时是封面派生出来的那一套） */
export function livePalette(): Palette {
  return current;
}

/** 换一套配色并通知三处。传进来的必须是已解析好的 Palette，不能用 id。 */
export function setLivePalette(palette: Palette): void {
  current = palette;
  for (const listener of listeners) listener();
}

/** 订阅配色变化；返回取消订阅的函数 */
export function subscribeLivePalette(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
