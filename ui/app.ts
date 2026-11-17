import "./app/root-mount.ts";
import "./app/startup-before-main.ts";
import "./app/main.ts";
import { startAppearance } from "./features/settings/appearance.ts";
import { initializeLyrics } from "./features/immersive/lyrics.ts";
import { startMiniPlayerHost } from "./mini/mini-player-host.ts";
import { startTaskbarBarHost } from "./taskbar-bar-host.ts";
import { initializeMainWindowReady } from "./app/main-window-ready.ts";
import { SURFACE_MODE_KEY, readString } from "./shared/storage.ts";
startAppearance();
initializeLyrics();
startMiniPlayerHost();
startTaskbarBarHost();
initializeMainWindowReady();

/** 启动时按上次选的「显示方式」打开对应窗口。
 *  以前这段逻辑在 Rust 的 setup 里无条件开任务栏条——不管用户选的是小组件还是任务栏，
 *  结果启动出来的是条、界面上高亮的却是小组件，两个状态对不上。
 *  现在只有这一处决定，模式是唯一事实来源。 */
function restoreSurfaceOnStartup(): void {
  const mode = readString(SURFACE_MODE_KEY);
  if (mode !== "bar" && mode !== "mini") {
    return;
  }
  const invoke = (window as unknown as { __TAURI__?: { core?: { invoke?: Function } } })
    .__TAURI__?.core?.invoke;
  if (!invoke) {
    return;
  }
  // 等主窗口渲染完再开，否则可能被主窗口的初始化抢焦点
  setTimeout(() => {
    const call = mode === "bar" ? invoke("open_taskbar_bar") : invoke("open_mini_player");
    Promise.resolve(call).catch((error: unknown) => {
      console.warn("[surface] 启动恢复显示方式失败", error);
    });
  }, 1200);
}

restoreSurfaceOnStartup();
