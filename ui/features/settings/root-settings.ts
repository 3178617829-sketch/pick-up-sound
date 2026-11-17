
type SettingsCallbacks = {
  /** 外观「主题」五选一：dark / light / picked / follow / image，一个入口定三种表现 */
  themeSource(value: string): void;
  theme(value: string): void; backgroundMode(value: string): void; accent(value: string): void;
  slider(kind: "blur" | "alpha" | "dim", value: string): void;
  /** 松手才落盘：拖动过程只改内存里的值，避免每次 input 都写 localStorage 导致卡顿 */
  sliderCommit(kind: "blur" | "alpha" | "dim", value: string): void;
  chooseBackground(): void; resetBackground(): void; saveAi(): void; testAi(): void; aiFormat(): void;
  source(value: string): void; cache(): void; collection(kind: "order" | "limit", value: string): void;
  clearCache(): void; clearHistory(): void; exportData(): void; importData(): void; purge(): void;
  shortcut(action: keyof CommandContract.ShortcutBindings): void; clearShortcut(action: keyof CommandContract.ShortcutBindings): void; normalization(enabled: boolean): void;
  /** 任务栏 / 小组件 / 都不显示；与侧栏底部那组按钮同一个入口 */
  surface(mode: "bar" | "mini" | "off"): void;
};
const settingsHandlers: Partial<SettingsCallbacks> = {};
const settingsCallbacks = Object.freeze<SettingsCallbacks>({
  themeSource: value => settingsHandlers.themeSource?.(value), theme: value => settingsHandlers.theme?.(value), backgroundMode: value => settingsHandlers.backgroundMode?.(value), accent: value => settingsHandlers.accent?.(value), slider: (kind, value) => settingsHandlers.slider?.(kind, value), sliderCommit: (kind, value) => settingsHandlers.sliderCommit?.(kind, value),
  chooseBackground: () => settingsHandlers.chooseBackground?.(), resetBackground: () => settingsHandlers.resetBackground?.(), saveAi: () => settingsHandlers.saveAi?.(), testAi: () => settingsHandlers.testAi?.(), aiFormat: () => settingsHandlers.aiFormat?.(),
  source: value => settingsHandlers.source?.(value), cache: () => settingsHandlers.cache?.(), collection: (kind, value) => settingsHandlers.collection?.(kind, value), clearCache: () => settingsHandlers.clearCache?.(), clearHistory: () => settingsHandlers.clearHistory?.(),
  exportData: () => settingsHandlers.exportData?.(), importData: () => settingsHandlers.importData?.(), purge: () => settingsHandlers.purge?.(), shortcut: action => settingsHandlers.shortcut?.(action), clearShortcut: action => settingsHandlers.clearShortcut?.(action),
  normalization: enabled => settingsHandlers.normalization?.(enabled),
  surface: mode => settingsHandlers.surface?.(mode),
});
function installSettingsCallbacks(next: Partial<SettingsCallbacks>): void { Object.assign(settingsHandlers, next); }
let trackColumnInitialization = () => {};
function installTrackColumnInitialization(initialize: () => void): void { trackColumnInitialization = initialize; }
function initializeTrackColumnPreferences(): void { trackColumnInitialization(); }
export { installSettingsCallbacks, settingsCallbacks, installTrackColumnInitialization, initializeTrackColumnPreferences };
export type { SettingsCallbacks };
