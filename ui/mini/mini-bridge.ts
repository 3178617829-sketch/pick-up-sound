/*
 * 小组件窗口对 Tauri 与浏览器全局的唯一出口。
 * 组件里不许直接摸 window / __TAURI__ / invoke —— scripts/vue/check.ts 会拦。
 */
type TauriBridge = {
  core?: { invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown> };
  window?: { getCurrentWindow?: () => { startResizeDragging?: (direction: string) => Promise<void> } };
};

function bridge(): TauriBridge | undefined {
  return (globalThis as unknown as { __TAURI__?: TauriBridge }).__TAURI__;
}

/** 内容缩放：把当前缩放比例告诉 Rust，窗口跟着收放 */
export function miniSetScale(scale: number): void {
  try {
    void bridge()?.core?.invoke?.("set_mini_scale", { scale })?.catch?.(() => {});
  } catch {
    /* 旧版内核没有这个命令，保持原尺寸即可 */
  }
}

/** 按住边角时把缩放交给系统（最跟手） */
export function miniStartResize(direction: string): void {
  try {
    void bridge()?.window?.getCurrentWindow?.()?.startResizeDragging?.(direction)?.catch?.(() => {});
  } catch {
    /* 不支持时保持原尺寸 */
  }
}

/** 当前可视区尺寸（用文档元素算，避开 window 标识符） */
export function miniViewport(): { width: number; height: number } {
  const root = document.documentElement;
  return { width: root.clientWidth, height: root.clientHeight };
}

export function miniOn(
  type: "wheel" | "resize" | "storage",
  handler: (event: Event) => void,
  options?: AddEventListenerOptions,
): void {
  if (typeof window === "undefined") return;
  window.addEventListener(type, handler as EventListener, options);
}

export function miniOff(type: "wheel" | "resize" | "storage", handler: (event: Event) => void): void {
  if (typeof window === "undefined") return;
  window.removeEventListener(type, handler as EventListener);
}

export function miniDelay(fn: () => void, ms: number): number {
  return globalThis.setTimeout(fn, ms);
}

export function miniEvery(fn: () => void, ms: number): number {
  return globalThis.setInterval(fn, ms);
}

export function miniStop(id: number): void {
  globalThis.clearInterval(id);
}
