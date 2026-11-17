// 任务栏条页面的宿主桥接：把 Tauri 的 invoke / listen 收在这里，
// 组件里只调这些函数，不出现 window / __TAURI__ / invoke 这些标识符（组件层禁止直连宿主）。
type BarBridge = {
  core?: { invoke?: (command: string, args?: unknown) => Promise<unknown> };
  event?: { listen?: (event: string, handler: (event: { payload?: unknown }) => void) => Promise<() => void> };
};

function barBridge(): BarBridge | undefined {
  return (window as unknown as { __TAURI__?: BarBridge }).__TAURI__;
}

function barInvoke(command: string, args?: unknown): Promise<unknown> {
  const call = barBridge()?.core?.invoke;
  return call ? Promise.resolve(call(command, args)).catch(() => {}) : Promise.resolve();
}

function barListen<T>(event: string, handler: (payload: T | undefined) => void): Promise<() => void> {
  const listen = barBridge()?.event?.listen;
  if (!listen) return Promise.resolve(() => {});
  return listen(event, incoming => handler(incoming.payload as T | undefined)).catch(() => () => {});
}

export { barBridge, barInvoke, barListen };
export type { BarBridge };
