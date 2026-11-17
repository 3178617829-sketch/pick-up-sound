import { attachSettingsVm } from "./helpers/settings-vm.ts";
import { sourceSlice } from "./helpers/source-slice.ts";
import assert from "node:assert/strict";
import { readFileSync } from "./helpers/module-syntax.ts";
import path from "node:path";
import { test } from "node:test";
import vm from "node:vm";

const source = readFileSync(path.join(import.meta.dirname, "../ui/features/settings/appearance.ts"), "utf8");
const storageSource = readFileSync(path.join(import.meta.dirname, "../ui/shared/storage.ts"), "utf8");
// 主题/音量 key 已收敛到 storage.ts，VM 里要的常量从那边取（export const 只剥前缀，声明仍可用）
// storage.ts 整份载入：常量与原语（readString/readFlag 等）都在里面，切片不能停在 readString 之前
const code = sourceSlice(storageSource, "ui/shared/storage.ts", null, null)
  + sourceSlice(source, "ui/features/settings/appearance.ts", "function readAccentColor(", "function resetBackground(");
const expected: Record<string, Record<string, number[]>> = {
  // pink 的 dark 是木棕色、light 是浅棕（与 orange.light 同值），名字只是个 id。
  // 2026-11-01 用户拍板：保留木棕色，不改成真粉色。两档色相不同是有意的，
  // 所以下面的色相一致断言对 pink 单独放行。
  pink: { dark: [192, 130, 60], light: [128, 77, 32] },
  blue: { dark: [122, 166, 231], light: [40, 88, 161] },
  purple: { dark: [189, 145, 236], light: [120, 48, 194] },
  green: { dark: [87, 185, 65], light: [40, 102, 25] },
  orange: { dark: [224, 146, 77], light: [128, 77, 32] },
  cyan: { dark: [73, 178, 191], light: [27, 98, 106] },
};
const colorKey = "bilibili-music.accent-color";

function setup(color = "pink", failWrites = false) {
  let notify: (() => void) | undefined;
  const stored = new Map([[colorKey, color]]);
  const writes: [string, string][] = [];
  const root: { dataset: { theme: string }; style: Record<string, unknown> } = {
    dataset: new Proxy<{ theme: string } & Record<PropertyKey, unknown>>({ theme: "dark" }, { set(target, key, value) {
      target[key] = value;
      if (key === "theme") queueMicrotask(() => notify?.());
      return true;
    } }),
    style: {
      "--dynamic-color-1": "rgb(28, 32, 40)",
      setProperty(this: Record<string, unknown>, key: string, value: string) { this[key] = value; },
      removeProperty(this: Record<string, unknown>, key: string) { delete this[key]; },
    },
  };
  const options: (EventTarget & { value: string; checked: boolean; nextElementSibling: { style: Record<string, string> } })[] = Object.keys(expected).map(value => Object.assign(new EventTarget(), {
    value, checked: false, nextElementSibling: { style: {} },
  }));
  const app = vm.createContext({
    root, accentColorOptions: options, themeOptions: [], imageOnlyGroups: [],
    glassBlurSlider: {}, glassBlurValue: {}, panelAlphaSlider: {}, panelAlphaValue: {},
    backgroundDimSlider: {}, backgroundDimValue: {},
    MutationObserver: class {
      constructor(callback: () => void) { notify = callback; }
      observe(target: unknown, options: unknown) {
        assert.equal(target, root);
        assert.deepEqual(JSON.parse(JSON.stringify(options)), { attributes: true, attributeFilter: ["data-theme"] });
      }
    },
    localStorage: {
      getItem(key: string) { return stored.get(key) ?? null; },
      setItem(key: string, value: string) {
        if (failWrites) throw Error("Unavailable");
        writes.push([key, value]); stored.set(key, value);
      },
    },
  });
  attachSettingsVm(app);
  vm.runInContext(code, app);
  app.initializeAccentColor();
  return { app, root, stored, writes, options };
}

function check(state: ReturnType<typeof setup>, color: string, theme: string) {
  const variant = theme === "light" ? "light" : "dark";
  // --accent-r/g/b 现在由配色层（applyPaletteToRoot）统一写，这里直接校验纯函数输出，
  // 不再依赖 settings-view 往 root 上写一次强调色（那条路已删，免得覆盖配色的精确色）。
  assert.deepEqual(Array.from<number>(state.app.accentChannels(color, theme)), expected[color][variant]);
  assert.deepEqual(state.options.filter(o => o.checked).map(o => o.value), [color]);
  for (const option of state.options) {
    assert.equal(option.nextElementSibling.style.backgroundColor, `rgb(${expected[option.value][variant].join(", ")})`);
  }
}

test("six names map to exact RGB variants in all four themes with safe fallbacks", () => {
  const { app } = setup();
  for (const [name, pair] of Object.entries(expected)) {
    for (const theme of ["dark", "light", "image", "dynamic", null, "unknown"]) {
      assert.deepEqual(Array.from<number>(app.accentChannels(name, theme)), pair[theme === "light" ? "light" : "dark"]);
    }
  }
  for (const value of [null, "unknown", "__proto__"]) {
    assert.deepEqual(Array.from<number>(app.accentChannels(value, "light")), expected.pink.light);
    assert.deepEqual(Array.from<number>(app.accentChannels(value, "dark")), expected.pink.dark);
  }
});

test("twelve actual variants meet contrast targets and pairs preserve hue within one degree", () => {
  const { app } = setup();
  const luminance: (rgb: number[]) => number = rgb => rgb.map(v => {
    v /= 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  }).reduce((sum: number, v: number, i: number) => sum + v * [0.2126, 0.7152, 0.0722][i], 0);
  const contrast = (a: number[], b: number[]) => (Math.max(luminance(a), luminance(b)) + 0.05) / (Math.min(luminance(a), luminance(b)) + 0.05);
  const mix = (a: number[], b: number[], alpha: number) => a.map((v: number, i: number) => v * alpha + b[i] * (1 - alpha));
  // Reuse the existing RGB/HSL implementation without initializing the background.
  const dynamic = readFileSync(path.join(import.meta.dirname, "../ui/features/settings/dynamic-background.ts"), "utf8");
  vm.runInContext(sourceSlice(dynamic, "ui/features/settings/dynamic-background.ts", null, "function initializeDynamicBackground"), app);
  for (const name of Object.keys(expected)) {
    const dark = Array.from<number>(app.accentChannels(name, "dark"));
    const light = Array.from<number>(app.accentChannels(name, "light"));
    // pink 的两档是两种棕（见文件头注释），不参与色相一致断言；其余五个必须同色相。
    if (name !== "pink") {
      assert.ok(app.hueDistance(app.rgbToHsl(dark)[0], app.rgbToHsl(light)[0]) < 1, name);
    }
    for (const [color, base, overlay, alpha] of [
      [dark, [28, 28, 30], [255, 255, 255], 0.024],
      [light, [242, 242, 244], [0, 0, 0], 0.025],
    ] as [number[], number[], number[], number][]) {
      assert.ok(contrast(color, base) >= 4.5, name);
      for (const panel of [base, mix(overlay, base, alpha)]) {
        assert.ok(contrast(color, mix(color, mix(color, panel, 0.14), 0.14)) >= 3, name);
      }
    }
  }
});

test("startup saved-light restoration and rapid theme changes update through microtasks without color writes", async () => {
  const state = setup("blue");
  state.app.applyTheme("light", false);
  await Promise.resolve();
  check(state, "blue", "light");
  state.app.applyTheme("dark", false);
  state.app.applyTheme("light", false);
  await Promise.resolve();
  check(state, "blue", "light");
  assert.deepEqual(state.writes, []);
  assert.equal(state.stored.get(colorKey), "blue");
});

test("switching themes updates selected color and all swatches without affecting dynamic palette or color storage", async () => {
  const state = setup();
  for (const name of Object.keys(expected)) {
    const option = state.options.find(o => o.value === name);
    if (option === undefined) throw new Error(`Missing color option: ${name}`);
    option.dispatchEvent(new Event("change"));
    for (const theme of ["dark", "light"]) {
      state.app.applyTheme(theme);
      await Promise.resolve();
      check(state, name, theme);
      assert.equal(state.stored.get(colorKey), name);
      assert.equal(state.root.style["--dynamic-color-1"], "rgb(28, 32, 40)");
    }
  }
  assert.equal(state.writes.filter(([key]) => key === colorKey).length, 6);
  // 「黑主题/白主题」现在同时就是一次配色选择，所以除颜色外只该写主题来源与它带出的那套配色。
  const allowed = new Set([colorKey, "bilibili-music.theme", "bilibili-music.theme-source", "mini-palette", "mini-skin"]);
  assert.ok(state.writes.every(([key]) => allowed.has(key)));
});

test("theme variants keep the current session color even when its persistence failed", async () => {
  const state = setup("pink", true);
  state.app.applyAccentColor("purple");
  state.app.applyTheme("light", false);
  await Promise.resolve();
  check(state, "purple", "light");
  state.app.applyBackgroundMode("cover", false);
  await Promise.resolve();
  check(state, "purple", "dynamic");
  assert.equal(state.stored.get(colorKey), "pink");
});
