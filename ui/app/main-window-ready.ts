import { invoke } from "../platform/runtime-api.ts";
import { setRootData } from "../shared/root-style.ts";

function initializeMainWindowReady(): void {
  window.addEventListener("DOMContentLoaded", () => {
    const startupRoot = document.documentElement;
    if (startupRoot.dataset.startupFailed === "true") return;
    setRootData("startupComplete", "true");
    window.onerror = null;
    void invoke("main_window_ready").catch(error => {
      console.error("main window ready failed:", error);
    });
  }, { once: true });
}

export { initializeMainWindowReady };
