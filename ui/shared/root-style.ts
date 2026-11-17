/**
 * `<html>` 元素上属性与 CSS 变量的唯一投影出口。
 *
 * 原生写守卫（scripts/vue/native-writes.ts）只放行本文件，业务代码一律走这里 ——
 * 这样「往 documentElement 上写东西」只有一个地方能写，白名单不必跟着每个调用点增长。
 *
 * 为什么不直接 `document.documentElement.style.setProperty(...)`：
 * 那会把「这处写入是干什么的」摊在每个调用点，改一处 CSS 变量要回来补一行白名单。
 *
 * `document` 在非浏览器环境（node 测试）可能不存在，所以每个入口都做存在性判断，
 * 缺了就是空操作，不抛。
 */

type RootElement = HTMLElement;

function rootElement(): RootElement | null {
  return typeof document === "undefined" ? null : document.documentElement;
}

/** 设置 `<html data-{name}="{value}">` */
function setRootData(name: string, value: string): void {
  const root = rootElement();
  if (root) root.dataset[name] = value;
}

/** 删除 `<html>` 上的 `data-{name}` 属性 */
function deleteRootData(name: string): void {
  const root = rootElement();
  if (root) delete root.dataset[name];
}

/** 设置 `<html>` 上的 CSS 自定义属性（`--accent`、`--background-dim` 之类） */
function setRootVar(name: string, value: string): void {
  const root = rootElement();
  if (root) root.style.setProperty(name, value);
}

/** 删除 `<html>` 上的 CSS 自定义属性 */
function removeRootVar(name: string): void {
  const root = rootElement();
  if (root) root.style.removeProperty(name);
}

export { deleteRootData, removeRootVar, setRootData, setRootVar };
