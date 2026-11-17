import { surfaceTheme } from "../features/settings/settings-state.ts";
import type { createApplicationStore } from "../store/app-state.ts";
import { activeQueueTrack } from "../features/playback/playback-queue-state.ts";
import { fixAccent, solidOf } from "../shared/palettes.ts";
import { livePalette } from "../shared/live-palette.ts";

function selectMiniPlayerSnapshot(state: ReturnType<ReturnType<typeof createApplicationStore>["getState"]>, dynamicColors?: readonly string[], imageScheme: "light"|"dark" = "dark") {
  const current = activeQueueTrack(state.playbackQueue);
  // 小组件的颜色跟主窗同一个来源（live 色源），不再用六个色名反推 RGB ——
  // 那样选了 tiffany / burgundy 这类配色时，小组件永远只是近似色，跟主窗对不上。
  const palette = livePalette();
  const accent = fixAccent(palette);
  const track = state.playbackDisplay.renderedTrack;
  return {
    title: track.title || "尚未播放", uploader: track.uploader || "—", thumbnailUrl: track.thumbnailUrl || "",
    hasCurrent: Boolean(current?.bvid), canPrevious: true, canNext: true,
    isPlaying: state.playbackDisplay.buttonPlaying,
    isFavorited: Boolean(current?.bvid && state.library.favoriteBvids.has(current.bvid.toLowerCase())),
    ...(surfaceTheme(state.settings) === "image" ? { imageScheme, backgroundPath: state.settings.background?.path ?? null, backgroundDim: state.settings.backgroundDim, backgroundVersion: state.settings.view.backgroundVersion } : {}),
    ...(surfaceTheme(state.settings) === "dynamic" && dynamicColors ? { dynamicColors } : {}),
    theme: surfaceTheme(state.settings), accent: { r: accent[0], g: accent[1], b: accent[2] },
    paletteBg: solidOf(palette.barBg), paletteText: palette.barText, glassBlur: state.settings.glassBlur,
  };
}
function selectMiniPlayerMessages(state: ReturnType<ReturnType<typeof createApplicationStore>["getState"]>) {
  return { status: state.playbackMessages.status, notice: state.playbackMessages.notice.text };
}
export { selectMiniPlayerMessages, selectMiniPlayerSnapshot };
