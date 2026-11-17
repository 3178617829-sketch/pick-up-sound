import { getApplicationStore } from "./features/home/home.ts";
import { getPlaybackCommands } from "./features/settings/appearance.ts";
import { fixAccent } from "./shared/palettes.ts";
import { livePalette } from "./shared/live-palette.ts";
// tauri-specta 生成的绑定：命令名与 8 个参数由 Rust 签名产出，拼错就是类型错误。
// `src-tauri/src/bindings.rs` 的测试会校验它与 Rust 侧是否同步。
import { commands as barApi } from "./shared/bindings/bar.ts";

const BAR_LABEL = "bar";
/** 歌词最多每 250ms 推一次，避免播放时把 IPC 打满。 */
const PUBLISH_THROTTLE_MS = 250;

type BarSnapshot = {
  title: string;
  uploader: string;
  lyric: string;
  playing: boolean;
  progress: number;
  /** 三处共用的配色：强调色 / 底色 / 文字色，都用 "r,g,b" 串传，原生 GDI 直接解析。 */
  accent: string;
  barBg: string;
  barText: string;
};

function rgbString(rgb: [number, number, number]): string {
  return `${rgb[0]},${rgb[1]},${rgb[2]}`;
}

function selectBarSnapshot(state: ReturnType<ReturnType<typeof getApplicationStore>["getState"]>): BarSnapshot {
  const display = state.playbackDisplay;
  const lyrics = state.lyricsDisplay;
  const active = lyrics.activeIndex >= 0 ? lyrics.lines[lyrics.activeIndex] : undefined;
  const text = active?.text?.trim() || lyrics.placeholder || "";
  const total = display.progress?.duration ?? 0;
  const position = display.progress?.position ?? 0;
  const palette = livePalette();
  const accent = fixAccent(palette);
  return {
    title: display.renderedTrack.title || "尚未播放",
    uploader: display.renderedTrack.uploader || "",
    lyric: text,
    playing: Boolean(display.buttonPlaying),
    progress: total > 0 ? Math.min(1, Math.max(0, position / total)) : 0,
    accent: rgbString(accent),
    barBg: rgbString(palette.barBgRgb),
    barText: rgbString(palette.barTextRgb),
  };
}

type TauriBridge = {
  event?: { listen?: (event: string, handler: (event: { payload?: { action?: string } }) => void) => Promise<unknown> };
  core?: { invoke?: (command: string, args?: unknown) => Promise<unknown> };
};

function startTaskbarBarHost() {
  const tauri = (window as unknown as { __TAURI__?: TauriBridge }).__TAURI__;
  const core = tauri?.core, eventApi = tauri?.event;
  if (!core?.invoke || !eventApi?.listen) {
    return;
  }
  const listen = eventApi.listen;
  const store = getApplicationStore();
  const commands = () => getPlaybackCommands();

  let lastKey = "";
  let timer: number | null = null;

  function publish() {
    const snapshot = selectBarSnapshot(store.getState());
    const key = `${snapshot.title}|${snapshot.lyric}|${snapshot.playing}|${snapshot.accent}|${snapshot.barBg}|${snapshot.barText}`;
    if (key === lastKey) {
      return;
    }
    lastKey = key;
    void barApi
      .push_bar_state(
        snapshot.title,
        snapshot.uploader,
        snapshot.lyric,
        snapshot.playing,
        snapshot.progress,
        snapshot.accent,
        snapshot.barBg,
        snapshot.barText,
      )
      .catch(() => {});
  }

  function schedule() {
    if (timer !== null) {
      return;
    }
    timer = window.setTimeout(() => {
      timer = null;
      publish();
    }, PUBLISH_THROTTLE_MS);
  }

  store.subscribe(schedule);

  void Promise.resolve(
    listen("bar-control", (event: { payload?: { action?: string } }) => {
      const action = event.payload?.action;
      if (action === "previous") {
        commands().previous();
      } else if (action === "play_pause" || action === "toggle_play") {
        void commands().togglePlayPause();
      } else if (action === "next") {
        commands().next();
      }
    }),
  ).catch(() => {});

  // 这里以前无条件 invoke("open_taskbar_bar")，导致无论用户选的是「小组件」还是
  // 「任务栏」，一启动都会冒出任务栏条。开窗改由 app.ts 的 restoreSurfaceOnStartup
  // 统一按 mini-surface-mode 决定；这里只负责推状态。
  // 条后开的话，主窗口这边订阅着 store，下一次状态变化就会推过去。
}

export { startTaskbarBarHost };
