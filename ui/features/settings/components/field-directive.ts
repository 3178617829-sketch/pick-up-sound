import type {SettingsControl} from "../settings-controls.ts";
// 只做显式写值：无关属性更新不许覆盖用户正在输入的原生草稿。
// 拖动滑杆时该 input 就是 activeElement——此时绝不能回写 value，否则滑块会被 store 拽回去
// （表现就是「拖太快位置和数字对不上 / 拖动闪烁 / 松手跳回」）。
function isFocused(element: unknown): boolean {
  const active = typeof document === "undefined" ? null : document.activeElement;
  return active !== null && active !== undefined && (active as unknown) === element;
}
function applySettingsField(element: { value: string; checked?: boolean }, value: SettingsControl, previous?: SettingsControl): void {
  if (value.valueVersion && value.valueVersion !== previous?.valueVersion && !isFocused(element)) element.value = value.value ?? "";
  if (value.checkedVersion && value.checkedVersion !== previous?.checkedVersion) element.checked = value.checked ?? false;
}
export {applySettingsField};
