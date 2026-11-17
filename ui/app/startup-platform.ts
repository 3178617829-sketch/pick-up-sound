import { setRootData } from "../shared/root-style.ts";
import { initializeSidebar } from "./sidebar.ts";

function initializeStartupPlatform(): void {
  setRootData("platform", /Macintosh|Mac OS X/i.test(navigator.userAgent) ? "macos" : "windows");
  window.onerror = () => {
    setRootData("startupFailed", "true");
  };
  initializeSidebar();
}

export { initializeStartupPlatform };
