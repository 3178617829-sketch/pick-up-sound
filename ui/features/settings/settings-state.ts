import type { DeepReadonly } from "../../store/state-store.ts";
import { normalizeShuffleCollectionPrefs } from "../pages/page-selection.ts";
import { activateCustomPalette, activePaletteId, applyPaletteToRoot, deactivateCustomPalette, isLightPalette, paletteById, readCustomPalette, PALETTE_KEY, SKIN_KEY } from "../../shared/palettes.ts";
import { setLivePalette } from "../../shared/live-palette.ts";
import { paletteFromImage } from "../../shared/custom-palette.ts";
const ACCENT_COLORS = {
  pink: {
    dark: [192, 130, 60], light: [128, 77, 32]
  },
  blue: {
    dark: [122, 166, 231], light: [40, 88, 161]
  },
  purple: {
    dark: [189, 145, 236], light: [120, 48, 194]
  },
  green: {
    dark: [87, 185, 65], light: [40, 102, 25]
  },
  orange: {
    dark: [224, 146, 77], light: [128, 77, 32]
  },
  cyan: {
    dark: [73, 178, 191], light: [27, 98, 106]
  },
};
function normalizeSettingsNumber(value: unknown, min: number, max: number, fallback: number) {
  if (value === null || value === undefined || value === "") {
    return fallback;
  }
  const number = Number(value);
  if (!Number.isFinite(number)) {
    return fallback;
  }
  return Math.min(Math.max(number, min), max);
}
type SettingsCommand = "get_yt_dlp_availability" | "get_stream_source" | "set_stream_source" | "choose_background_image" | "load_background_image" | "get_audio_cache_settings" | "set_audio_cache_settings" | "get_audio_cache_usage" | "clear_audio_cache" | "get_shortcuts" | "set_shortcuts" | "get_ai_config" | "set_ai_config" | "test_ai_connection";
type SettingsPlatform = {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
  cancelCacheNotice(): void;
  scheduleCacheNotice(callback: () => void): void
};
type SettingsAdapter = {
  invoke: <K extends SettingsCommand>(command: K, ...args: CommandArguments<K>) => Promise<CommandMap[K]["result"]>;
  report(level: "warn" | "error", message: string, error: unknown): void;
  settings?: SettingsPlatform
};
type AiInput = {
  apiFormat: string;
  baseUrl: string;
  model: string;
  apiKey: string
};
type ShortcutAction = keyof CommandContract.ShortcutBindings;
/** 背景轴：纯色 / 一张图 / 跟着封面变。明暗另存 theme，两轴正交 */
type BackgroundMode = "solid" | "image" | "cover";
/**
 * 外观来源 —— 设置页「主题」那一行五选一，互斥：
 *   dark   黑主题（wood 铜金木纹）
 *   light  白主题（paper 白玻璃）
 *   picked 从 40 套里手选一套
 *   follow 跟着封面色自动挑
 *   image  自定义图片（图定的底 + 图取的色）
 * 以前是「明暗」+「背景」+「颜色」三条轴各写各的，选白主题再挑配色会互相覆盖，
 * 所以收成一个来源：选哪个就哪个说了算，其余四种在界面上点不动。
 * theme / backgroundMode 两个老轴保留，由 setThemeSource 一并推导写回 —— CSS 与迷你窗只认它们。
 */
type ThemeSource = "dark" | "light" | "picked" | "follow" | "image";
const THEME_SOURCES: readonly ThemeSource[] = ["dark", "light", "picked", "follow", "image"];
function normalizeThemeSource(value: unknown): ThemeSource {
  return typeof value === "string" && (THEME_SOURCES as readonly string[]).includes(value) ? value as ThemeSource : "dark";
}
type SettingsState = {
  controls: { dataBusy: { backupExport: boolean; backupImport: boolean; history: boolean } };
  themeSource: ThemeSource;
  theme: string | null;
  backgroundMode: BackgroundMode;
  accentColor: keyof typeof ACCENT_COLORS;
  glassBlur: number;
  panelAlpha: number;
  backgroundDim: number;
  background: CommandContract.BackgroundImage | null;
  ytDlpAvailable: boolean;
  streamSource: string;
  audioCacheSettings: CommandContract.AudioCacheSettings | null;
  collection: ReturnType<typeof normalizeShuffleCollectionPrefs>;
  shortcutBindings: CommandContract.ShortcutBindings;
  recordingShortcutAction: ShortcutAction | null;
  ai: {
    apiFormat: string;
    baseUrl: string;
    model: string;
    hasKey: boolean;
    request: "idle" | "loading" | "saving" | "testing"
  };
  session: {
    currentView: string | null;
    placeholderTitle: string;
    navigationVersion: number;
    panelOpen: boolean;
    panelVersion: number
  };
  view: {
    themeVersion: number;
    accent: {
      theme: string | null | undefined
    }
    | null;
    sliders: {
      kind: "all" | "blur" | "alpha" | "dim"
    }
    | null;
    backgroundVersion: number;
    notice: {
      text: string
    }
    | null;
    chooseDisabled: boolean;
    shortcuts: {
      action: ShortcutAction
    }
    | null;
    availabilityVersion: number;
    stream: {
      valueVersion: number;
      value: string | null;
      text: string | null;
      disabled: boolean | null
    };
    collectionVersion: number;
    cache: {
      settingsVersion: number;
      disabled: boolean | null;
      status: {
        text: string
      }
      | null;
      usageText: string | null;
      clearDisabled: boolean | null
    };
    ai: {
      fieldsVersion: number;
      clearKeyVersion: number;
      text: string | null;
      saveDisabled: boolean | null;
      testDisabled: boolean | null;
      placeholderVersion: number
    };
  };
};
type SettingsSnapshot = DeepReadonly<SettingsState>;
/** 背景遮罩默认百分比。设置面板滑杆的初始值、迷你播放器的兜底都取这一处——
 *  以前三处各写一个数（30 / 40 / 40），滑杆会先显示 40% 再被推成 30。 */
const DEFAULT_BACKGROUND_DIM = 30;
function initialSettingsState(): SettingsState {
  return {
    controls: {dataBusy: {backupExport:false, backupImport:false, history:false}},
    themeSource: "dark", theme: null, backgroundMode: "solid", accentColor: "pink", glassBlur: 40, panelAlpha: 72, backgroundDim: DEFAULT_BACKGROUND_DIM, background: null, ytDlpAvailable: false, streamSource: "auto", audioCacheSettings: null, collection: {
      order: "random", limit: 0
    }, shortcutBindings: {
      previous: null, playPause: null, next: null, volumeUp: null, volumeDown: null
    }, recordingShortcutAction: null,
    ai: {
      apiFormat: "openai-chat-completions", baseUrl: "", model: "", hasKey: false, request: "idle"
    }, session: {
      currentView: "home", placeholderTitle: "", navigationVersion: 0, panelOpen: false, panelVersion: 0
    },
    view: {
      themeVersion: 0, accent: null, sliders: null, backgroundVersion: 0, notice: null, chooseDisabled: false, shortcuts: null, availabilityVersion: 0, stream: {
        valueVersion: 0, value: null, text: null, disabled: null
      }, collectionVersion: 0, cache: {
        settingsVersion: 0, disabled: null, status: null, usageText: null, clearDisabled: null
      }, ai: {
        fieldsVersion: 0, clearKeyVersion: 0, text: null, saveDisabled: null, testDisabled: null, placeholderVersion: 0
      }
    }
  };
}
function normalizeSettingsAccent(value: unknown): keyof typeof ACCENT_COLORS {
  return typeof value === "string" && Object.hasOwn(ACCENT_COLORS, value) ? value as keyof typeof ACCENT_COLORS : "pink";
}
function accentChannels(color: unknown, theme: string | null | undefined) {
  return ACCENT_COLORS[normalizeSettingsAccent(color)][theme === "light" ? "light" : "dark"];
}
function redactAiText(value: unknown, keys: readonly string[]): string {
  let result = String(value);
  for (const key of keys) if (key) result = result.split(key).join("[已隐藏]");
  return result;
}
function createSettingsActions(read: () => SettingsSnapshot, commit: (next: SettingsSnapshot) => void, adapter: SettingsAdapter) {
  const key = (name: string) => "bilibili-music." + name;
  const update = (fields: Partial<SettingsSnapshot>, view: Partial<SettingsSnapshot["view"]> = {
  }) => commit({
    ...read(), ...fields, view: Object.keys(view).length ? {
      ...read().view, ...view
    } : read().view
  });
  const notice = (text: string) => update({
  }, {
    notice: {
      text
    }
  });
  const cacheView = (fields: Partial<SettingsSnapshot["view"]["cache"]>, state: Partial<SettingsSnapshot> = {
  }) => update(state, {
    cache: {
      ...read().view.cache, ...fields
    }
  });
  const streamView = (fields: Partial<SettingsSnapshot["view"]["stream"]>, state: Partial<SettingsSnapshot> = {
  }) => update(state, {
    stream: {
      ...read().view.stream, ...fields, valueVersion: read().view.stream.valueVersion + ("value" in fields ? 1 : 0)
    }
  });
  const aiView = (fields: Partial<SettingsSnapshot["view"]["ai"]>, state: Partial<SettingsSnapshot["ai"]> = {
  }) => update({
    ai: {
      ...read().ai, ...state
    }
  }, {
    ai: {
      ...read().view.ai, ...fields
    }
  });
  /** 某种来源该用哪套配色：黑主题钉住 wood、白主题钉住 paper，其余三种用存储里那套。 */
  function paletteForSource(source: ThemeSource) {
    if (source === "dark") return paletteById("wood");
    if (source === "light") return paletteById("paper");
    return paletteById(activePaletteId());
  }
  /** 五选一 → 老的两轴。CSS 与迷你窗只认 theme / backgroundMode，由这里一并写回。 */
  function axisForSource(source: ThemeSource, palette: ReturnType<typeof paletteById>): { theme: "dark" | "light"; backgroundMode: BackgroundMode } {
    if (source === "image") return { theme: "dark", backgroundMode: "image" };
    // 浅底配色（paper/creme/mint 等）配深色字：明暗轴跟着配色亮度走
    const theme: "dark" | "light" = isLightPalette(palette) ? "light" : "dark";
    return { theme, backgroundMode: source === "follow" ? "cover" : "solid" };
  }
  /**
   * 外观唯一入口：五选一。一次写回 themeSource、两个老轴、以及整套配色变量。
   * 三处（主窗 / 小组件 / 任务栏条）共用同一个 live 色源，调用方在切「手选」时另行 setLivePalette。
   */
  function setThemeSource(value: unknown, persist = true): void {
    const source = normalizeThemeSource(value);
    const palette = paletteForSource(source);
    const axis = axisForSource(source, palette);
    update({
      themeSource: source, theme: axis.theme, backgroundMode: axis.backgroundMode
    }, {
      themeVersion: read().view.themeVersion + 1
    });
    // 封面模式下遮罩与模糊由封面背景自己算，不拿存档值去覆盖
    if (axis.backgroundMode !== "cover") {
      setSlider("blur", adapter.settings?.getItem(key("glass-blur")), false);
      setSlider("dim", adapter.settings?.getItem(key("background-dim")), false);
    }
    applyPaletteToRoot(palette, surfaceTheme({ themeSource: source, theme: axis.theme }));
    // 三处（主窗 / 小组件 / 任务栏条）共用同一个 live 色源：这里一写，条和小组件跟着变
    setLivePalette(palette);
    if (persist) {
      adapter.settings?.setItem(key("theme-source"), source);
      // 黑/白/手选三种才记「上次选的哪套」；跟随歌曲的颜色由封面决定，图片主题由底图决定
      if (source === "dark" || source === "light" || source === "picked") {
        adapter.settings?.setItem(PALETTE_KEY, palette.id);
        adapter.settings?.setItem(SKIN_KEY, palette.skin);
      }
    }
  }
  /** 老调用点（appearance.ts 的 applyTheme）只传 dark / light：转成来源，不再单独改明暗轴。 */
  function setTheme(value: string | null, persist = true): void {
    const raw = value ?? "";
    if (raw === "image" || raw === "dynamic") {
      setThemeSource(raw === "image" ? "image" : "follow", persist);
      return;
    }
    setThemeSource(raw === "light" ? "light" : "dark", persist);
  }
  /** 老调用点（backgroundMode）传三值：image / cover 转来源，solid 退回黑或白。 */
  function setBackgroundMode(value: unknown, persist = true): void {
    if (value === "image") {
      setThemeSource("image", persist);
      return;
    }
    if (value === "cover") {
      setThemeSource("follow", persist);
      return;
    }
    const current = read().themeSource;
    if (current !== "image" && current !== "follow") {
      setThemeSource(current, persist);
      return;
    }
    // 从图片 / 封面退回纯色：明暗沿用当前那套配色，不跳回黑主题
    setThemeSource(read().theme === "light" ? "light" : "dark", persist);
  }
  /** 老配置迁移：theme + background-mode 两个 key → 一个 theme-source。只读不写。 */
  function storedThemeSource(): ThemeSource {
    const stored = adapter.settings?.getItem(key("theme-source"));
    if (stored) return normalizeThemeSource(stored);
    const mode = adapter.settings?.getItem(key("background-mode"));
    const theme = adapter.settings?.getItem(key("theme"));
    if (mode === "image" || theme === "image") return "image";
    if (mode === "cover" || theme === "dynamic") return "follow";
    const paletteId = adapter.settings?.getItem(PALETTE_KEY);
    if (paletteId === "custom") return "image";
    if (paletteId === "paper") return "light";
    if (paletteId && paletteId !== "wood") return "picked";
    return theme === "light" ? "light" : "dark";
  }
  function setAccent(value: unknown, theme: string | null | undefined, persist = true): void {
    const accentColor = normalizeSettingsAccent(value);
    update({
      accentColor
    }, {
      accent: {
        theme
      }
    });
    if (persist) try {
      adapter.settings?.setItem(key("accent-color"), accentColor);
    } catch {
    }
  }
  function setSlider(kind: "blur" | "alpha" | "dim", value: unknown, persist = true): void {
    if (kind === "alpha") return; // 隐藏兼容端口，不改写用户遗留值。
    const field = kind === "blur" ? "glassBlur" : "backgroundDim";
    const safe = kind === "blur" ? normalizeSettingsNumber(value, 0, 80, 40) : normalizeSettingsNumber(value, 0, 95, 30);
    update({
      [field]: safe
    }, {
      sliders: {
        kind
      }
    });
    if (persist) adapter.settings?.setItem(key(kind === "blur" ? "glass-blur" : "background-dim"), String(safe));
  }
  function applyImage(image: CommandContract.BackgroundImage): void {
    update({
      background: {
        ...image
      }
    }, {
      backgroundVersion: read().view.backgroundVersion + 1
    });
  }
  /** 从背景图取一套配色，让主窗、小组件、任务栏条同一个色源。 */
  async function deriveImagePalette(image: CommandContract.BackgroundImage): Promise<void> {
    const palette = await paletteFromImage(image.dataUrl);
    if (palette) {
      activateCustomPalette(palette);
      return;
    }
    notice("背景已保存，但这张图取不出可用颜色，配色保持原样。");
  }
  function resetSettingsBackground({
    clearStorage = true
  }
  = {
  }): void {
    update({
      background: null
    }, {
      backgroundVersion: read().view.backgroundVersion + 1, notice: {
        text: ""
      }
    });
    if (clearStorage) adapter.settings?.removeItem(key("background-path"));
    if (read().backgroundMode === "image") setBackgroundMode("solid");
    deactivateCustomPalette();
  }
  async function restoreSettingsBackground(): Promise<void> {
    const path = adapter.settings?.getItem(key("background-path"));
    if (!path) return;
    notice("正在恢复背景…");
    try {
      const image = await adapter.invoke("load_background_image", {
        path
      });
      applyImage(image);
      // 已经有一套手调 / 图取的配色就不要再从图里算一遍——
      // 手调过的颜色每次启动都被图覆盖，等于白调
      if (!readCustomPalette()) await deriveImagePalette(image);
      notice("");
    } catch (error) {
      resetSettingsBackground();
      notice(`背景已回退为默认：${error}`);
    }
  }
  async function chooseBackground(): Promise<void> {
    update({
    }, {
      chooseDisabled: true, notice: {
        text: "正在处理图片…"
      }
    });
    try {
      const image = await adapter.invoke("choose_background_image");
      if (!image) {
        notice("");
        return;
      }
      applyImage(image);
      adapter.settings?.setItem(key("background-path"), image.path);
      setBackgroundMode("image");
      await deriveImagePalette(image);
      notice("背景已保存。");
    } catch (error) {
      notice(`背景设置失败：${error}`);
    } finally {
      update({
      }, {
        chooseDisabled: false
      });
    }
  }
  async function refreshAvailability(): Promise<boolean> {
    let ytDlpAvailable = false;
    try {
      ytDlpAvailable = Boolean((await adapter.invoke("get_yt_dlp_availability"))?.available);
    } catch (error) {
      adapter.report("warn", "get_yt_dlp_availability failed:", error);
    }
    update({
      ytDlpAvailable
    }, {
      availabilityVersion: read().view.availabilityVersion + 1
    });
    return ytDlpAvailable;
  }
  const sourceLabel = (source: string) => source === "guest" ? "当前：游客直连" : "当前：自动（游客优先，yt-dlp 兜底）";
  const sourceHint = () => "自动模式需在程序目录放置 yt-dlp.exe 后启用";
  async function restoreSource(): Promise<void> {
    try {
      await refreshAvailability();
      let source = await adapter.invoke("get_stream_source");
      if (source === "yt-dlp" || source === "auto" && !read().ytDlpAvailable) source = await adapter.invoke("set_stream_source", {
        source: "guest"
      });
      const value = source === "auto" && read().ytDlpAvailable ? "auto" : "guest";
      streamView({
        value, text: source === "auto" && read().ytDlpAvailable ? sourceLabel("auto") : read().ytDlpAvailable ? sourceLabel("guest") : sourceLabel("guest") + "；" + sourceHint()
      }, {
        streamSource: source
      });
    } catch (error) {
      streamView({
        text: `取流方案读取失败：${error}`
      });
    }
  }
  async function changeSource(source: string): Promise<void> {
    if (source === "auto" && !read().ytDlpAvailable) {
      streamView({
        value: "guest", text: sourceLabel("guest") + "；" + sourceHint()
      }, {
        streamSource: "guest"
      });
      return;
    }
    streamView({
      disabled: true, text: "正在切换取流方案…"
    });
    try {
      const value = await adapter.invoke("set_stream_source", {
        source
      });
      streamView({
        value, text: sourceLabel(value)
      }, {
        streamSource: value
      });
    } catch (error) {
      streamView({
        text: `取流方案切换失败：${error}`
      });
      await restoreSource();
    } finally {
      streamView({
        disabled: false
      });
    }
  }
  async function restoreCache(): Promise<void> {
    adapter.settings?.cancelCacheNotice();
    cacheView({
      disabled: true
    }, {
      audioCacheSettings: null
    });
    try {
      const audioCacheSettings = await adapter.invoke("get_audio_cache_settings");
      cacheView({
        settingsVersion: read().view.cache.settingsVersion + 1, status: {
          text: ""
        }
      }, {
        audioCacheSettings: {
          ...audioCacheSettings
        }
      });
    } catch (error) {
      cacheView({
        status: {
          text: `缓存设置读取失败：${error}`
        }
      });
    } finally {
      cacheView({
        disabled: !read().audioCacheSettings
      });
    }
  }
  async function refreshCacheUsage(): Promise<void> {
    cacheView({
      usageText: "读取中…"
    });
    try {
      const {
        bytes, items
      }
      = await adapter.invoke("get_audio_cache_usage");
      const size = bytes === 0 ? "0 MB" : bytes < 1048576 ? "不足 1 MB" : `${(bytes / 1048576).toFixed(1)} MB`;
      cacheView({
        usageText: `${size} · ${items} 首`
      });
    } catch (error) {
      cacheView({
        usageText: `读取失败：${error}`
      });
    }
  }
  async function saveCache(enabled: boolean, maxBytes: number): Promise<void> {
    if (!read().audioCacheSettings) return;
    adapter.settings?.cancelCacheNotice();
    cacheView({
      disabled: true
    });
    try {
      const audioCacheSettings = await adapter.invoke("set_audio_cache_settings", {
        enabled, maxBytes
      });
      cacheView({
        settingsVersion: read().view.cache.settingsVersion + 1, status: {
          text: ""
        }
      }, {
        audioCacheSettings: {
          ...audioCacheSettings
        }
      });
      await refreshCacheUsage();
    } catch (error) {
      cacheView({
        settingsVersion: read().view.cache.settingsVersion + 1, status: {
          text: `缓存设置保存失败：${error}`
        }
      });
    } finally {
      cacheView({
        disabled: false
      });
    }
  }
  async function clearCache(): Promise<void> {
    adapter.settings?.cancelCacheNotice();
    cacheView({
      clearDisabled: true, status: {
        text: "正在清空缓存…"
      }
    });
    try {
      await adapter.invoke("clear_audio_cache");
      await refreshCacheUsage();
      cacheView({
        status: {
          text: "缓存已清空"
        }
      });
      adapter.settings?.scheduleCacheNotice(() => cacheView({
        status: {
          text: ""
        }
      }));
    } catch (error) {
      cacheView({
        status: {
          text: `清空缓存失败：${error}`
        }
      });
    } finally {
      cacheView({
        clearDisabled: false
      });
    }
  }
  function restoreCollection(order: unknown, limit: string | null): void {
    update({
      collection: normalizeShuffleCollectionPrefs(order, limit)
    }, {
      collectionVersion: read().view.collectionVersion + 1
    });
  }
  function saveCollection(name: "order" | "limit", value: string): void {
    const before = read().collection;
    update({
      collection: normalizeShuffleCollectionPrefs(name === "order" ? value : before.order, name === "limit" ? value : before.limit === 0 ? "all" : String(before.limit))
    });
    try {
      adapter.settings?.setItem(key("shuffle-collection-" + name), value);
    } catch (error) {
      notice(`随机播放设置保存失败：${error}`);
    }
  }
  function beginRecording(action: ShortcutAction): void {
    update({
      recordingShortcutAction: action
    }, {
      shortcuts: {
        action
      }, notice: {
        text: ""
      }
    });
  }
  function cancelRecording(): void {
    const action = read().recordingShortcutAction;
    if (action) update({
      recordingShortcutAction: null
    }, {
      shortcuts: {
        action
      }
    });
  }
  async function saveShortcut(action: ShortcutAction, value: string | null): Promise<void> {
    const bindings = {
      ...read().shortcutBindings, [action]: value
    };
    try {
      await adapter.invoke("set_shortcuts", {
        bindings
      });
      update({
        shortcutBindings: bindings
      }, {
        shortcuts: {
          action
        }, notice: {
          text: "快捷键已保存。"
        }
      });
    } catch (error) {
      notice(String(error));
    }
  }
  async function loadShortcuts(): Promise<void> {
    try {
      const result = await adapter.invoke("get_shortcuts");
      update({
        shortcutBindings: {
          ...read().shortcutBindings, ...result.bindings
        }
      });
    } catch (error) {
      adapter.report("warn", "[shortcut] cannot load settings:", error);
    }
  }
  function configText(config: CommandContract.AiConfigView): string {
    const hint = config.keyHint;
    return config.hasKey ? `API Key 已配置${hint ? `（${hint}）` : ""}` : "API Key 未配置";
  }
  function publicAi(input: {
    apiFormat?: string;
    baseUrl?: string;
    model?: string;
    hasKey?: boolean
  }, keys: readonly string[] = []) {
    return {
      apiFormat: redactAiText(input.apiFormat ?? "openai-chat-completions", keys), baseUrl: redactAiText(input.baseUrl ?? "", keys), model: redactAiText(input.model ?? "", keys), ...(input.hasKey !== undefined ? {
        hasKey: input.hasKey
      } : {
      })
    };
  }
  async function restoreAi(): Promise<void> {
    aiView({
      text: "正在读取 AI 配置…"
    }, {
      request: "loading"
    });
    try {
      const config = await adapter.invoke("get_ai_config");
      aiView({
        fieldsVersion: read().view.ai.fieldsVersion + 1, clearKeyVersion: read().view.ai.clearKeyVersion + 1, text: configText(config), placeholderVersion: read().view.ai.placeholderVersion + 1
      }, {
        ...publicAi(config), request: "idle"
      });
    } catch (error) {
      aiView({
        text: `AI 配置读取失败：${error}`
      }, {
        request: "idle"
      });
    }
  }
  async function saveAi(input: AiInput, latest: () => AiInput = () => input, onSaved: () => void = () => {
  }): Promise<boolean | undefined> {
    const keys = [input.apiKey];
    aiView({
      saveDisabled: true, text: "正在验证并保存 AI 配置…"
    }, {
      ...publicAi(input, keys), request: "saving"
    });
    try {
      const result = await adapter.invoke("test_ai_connection", input);
      if (!result.ok) {
        aiView({
          text: "连接测试未通过，未保存：" + redactAiText(result.message, keys)
        });
        return;
      }
      const next = latest();
      keys.push(next.apiKey);
      const config = await adapter.invoke("set_ai_config", next);
      aiView({
        clearKeyVersion: read().view.ai.clearKeyVersion + 1, text: "连接正常，配置已保存。"
      }, {
        hasKey: config.hasKey
      });
      onSaved();
      return true;
    } catch (error) {
      aiView({
        text: "AI 配置保存失败：" + redactAiText(error, keys)
      });
    } finally {
      aiView({
        saveDisabled: false
      }, {
        request: "idle"
      });
    }
  }
  async function testAi(input: AiInput): Promise<void> {
    aiView({
      testDisabled: true, text: "正在测试 AI 连接…"
    }, {
      ...publicAi(input, [input.apiKey]), request: "testing"
    });
    try {
      const result = await adapter.invoke("test_ai_connection", input);
      aiView({
        text: result.ok ? "AI 连接正常。" : "AI 连接失败：" + redactAiText(result.message, [input.apiKey])
      });
    } catch (error) {
      aiView({
        text: "AI 连接测试失败：" + redactAiText(error, [input.apiKey])
      });
    } finally {
      aiView({
        testDisabled: false
      }, {
        request: "idle"
      });
    }
  }
  function setAiFormat(apiFormat: string): void {
    aiView({
      placeholderVersion: read().view.ai.placeholderVersion + 1
    }, {
      apiFormat
    });
  }
  function navigate(currentView: string): void {
    const labels: Record<string, string> = {
      favorites: "我的收藏", playlists: "我的歌单", local: "本地音乐"
    };
    update({
      session: {
        ...read().session, currentView, placeholderTitle: `${labels[currentView] ?? "功能"}开发中`, navigationVersion: read().session.navigationVersion + 1
      }
    });
  }
  function panel(open: boolean): void {
    update({
      session: {
        ...read().session, panelOpen: open, panelVersion: read().session.panelVersion + 1
      }
    });
  }
  function setDataBusy(kind: "backupExport" | "backupImport" | "history", busy: boolean): void { update({controls:{...read().controls,dataBusy:{...read().controls.dataBusy,[kind]:busy}}}); }
  return {
    setDataBusy, setTheme, setThemeSource, storedThemeSource, setBackgroundMode, setAccent, setSlider, applyImage, resetBackground: resetSettingsBackground, restoreBackground: restoreSettingsBackground, chooseBackground, refreshAvailability, restoreSource, changeSource, restoreCache, refreshCacheUsage, saveCache, clearCache, restoreCollection, saveCollection, beginRecording, cancelRecording, saveShortcut, loadShortcuts, restoreAi, saveAi, testAi, setAiFormat, navigate, panel, setNotice: notice
  };
}
/** 把「主题来源」还原成旧的 data-theme 四值，CSS 与迷你窗沿用同一套选择器 */
function surfaceTheme(state: Pick<SettingsState, "themeSource" | "theme">): "dark" | "light" | "image" | "dynamic" {
  if (state.themeSource === "image") return "image";
  if (state.themeSource === "follow") return "dynamic";
  // 黑/白/手选三种都只看明暗轴（明暗轴由 setThemeSource 按配色亮度写回）
  return state.theme === "light" ? "light" : "dark";
}
export { DEFAULT_BACKGROUND_DIM, accentChannels, createSettingsActions, initialSettingsState, normalizeSettingsAccent, normalizeThemeSource, surfaceTheme };
export type { AiInput, SettingsCommand, SettingsPlatform, SettingsSnapshot, ThemeSource };

let imageForegroundPort: ((state: Pick<SettingsSnapshot, "theme" | "backgroundMode" | "backgroundDim" | "background">) => void) | undefined;
function installImageForegroundPort(port: NonNullable<typeof imageForegroundPort>): void { imageForegroundPort = port; }
function projectImageForeground(state: SettingsSnapshot): void { imageForegroundPort?.(state); }
export { installImageForegroundPort, projectImageForeground };
