/*
 * 设置面板要发的那几个界面事件。
 * 抽出来是因为 .vue 组件里不许直接摸 window（scripts/vue/check.ts 的守卫），
 * 而这几件事本质就是「告诉外面：主题 / 配色 / 跟随开关变了」。
 */

export function emitThemeChange(theme: string): void {
  window.dispatchEvent(new CustomEvent("theme-change", { detail: theme }));
}

export function emitPaletteChange(id: string): void {
  window.dispatchEvent(new CustomEvent("palette-change", { detail: id }));
}

export function emitFollowSongChange(following: boolean): void {
  window.dispatchEvent(new CustomEvent("follow-song-change", { detail: following }));
}
