/*
 * palettes.ts 的纯推导契约测试。
 *
 * 配色是三处（主窗口 / 迷你窗 / 任务栏条）共用的一份定义，`paletteById` 是唯一的
 * 查表入口。这里钉住两件事：
 *   1. 未知 id 必须回落到 `DEFAULT_PALETTE`，不能抛、不能返回 undefined
 *      （前端到处 `?.` 猜，一崩就是整块界面没颜色）；
 *   2. "custom" 这个 id 只在真的存在自定义配色时才生效，否则同样回落。
 *
 * 用 node:test + VM 切片，跟项目里其它测试一致，不引第三方框架。
 */
import assert from "node:assert/strict";
import path from "node:path";
import { test } from "node:test";
import vm from "node:vm";

import { readFileSync } from "./helpers/module-syntax.ts";
import { sourceSlice } from "./helpers/source-slice.ts";

const root = path.join(import.meta.dirname, "..");
const storageSource = readFileSync(path.join(root, "ui/shared/storage.ts"), "utf8");
const palettesSource = readFileSync(path.join(root, "ui/shared/palettes.ts"), "utf8");

interface PaletteLike {
  id: string;
  label: string;
  accent: number[];
  skin: string;
  barBg: string;
  barText: string;
  barBgRgb: number[];
  barTextRgb: number[];
}

interface PalettesApi {
  PALETTES: PaletteLike[];
  DEFAULT_PALETTE: string;
  CUSTOM_PALETTE_ID: string;
  paletteById(id: string | null | undefined): PaletteLike;
}

function load(store: Map<string, string> = new Map()): PalettesApi {
  const localStorage = {
    getItem: (key: string) => (store.has(key) ? store.get(key)! : null),
    setItem: (key: string, value: string) => void store.set(key, String(value)),
    removeItem: (key: string) => void store.delete(key),
  };
  const context = vm.createContext({
    localStorage,
    // palettes.ts 只在这些函数里碰 window，paletteById 不走它们；给个空壳防意外。
    window: { dispatchEvent: () => undefined },
    CustomEvent: class {
      constructor(_type: string) {}
    },
  });
  const storageCode = sourceSlice(storageSource, "ui/shared/storage.ts", null, null);
  const palettesCode = sourceSlice(palettesSource, "ui/shared/palettes.ts", null, null);
  const script = `${storageCode}\n${palettesCode}\n;({ PALETTES, DEFAULT_PALETTE, CUSTOM_PALETTE_ID, paletteById })`;
  return new vm.Script(script).runInContext(context) as PalettesApi;
}

test("DEFAULT_PALETTE 必须真的存在于 PALETTES 里（防拼错）", () => {
  const api = load();
  assert.ok(api.PALETTES.length >= 40, `期望至少 40 套配色，实际 ${api.PALETTES.length}`);
  assert.ok(
    api.PALETTES.some(p => p.id === api.DEFAULT_PALETTE),
    `DEFAULT_PALETTE "${api.DEFAULT_PALETTE}" 不在 PALETTES 里`,
  );
});

test("已知 id 返回对应那套", () => {
  const api = load();
  for (const wanted of [api.DEFAULT_PALETTE, api.PALETTES[api.PALETTES.length - 1].id]) {
    const got = api.paletteById(wanted);
    assert.equal(got.id, wanted);
  }
});

test("未知 id / 空值一律回落到 DEFAULT_PALETTE，不抛", () => {
  const api = load();
  for (const bad of ["no-such-palette", "", null, undefined]) {
    assert.equal(api.paletteById(bad).id, api.DEFAULT_PALETTE, `输入 ${String(bad)} 应回落`);
  }
});

test("每套配色的派生字段齐全（GDI 任务栏条要用 barBgRgb / barTextRgb）", () => {
  const api = load();
  for (const palette of api.PALETTES) {
    assert.match(palette.id, /^[a-z0-9-]+$/, `配色 id 不合法：${palette.id}`);
    assert.equal(palette.accent.length, 3, `${palette.id} 的 accent 应是三元组`);
    assert.equal(palette.barBgRgb.length, 3, `${palette.id} 缺 barBgRgb`);
    assert.equal(palette.barTextRgb.length, 3, `${palette.id} 缺 barTextRgb`);
    for (const channel of [...palette.accent, ...palette.barBgRgb, ...palette.barTextRgb]) {
      assert.ok(
        Number.isInteger(channel) && channel >= 0 && channel <= 255,
        `${palette.id} 有越界颜色分量：${channel}`,
      );
    }
    assert.ok(palette.barBg.length > 0 && palette.barText.length > 0, `${palette.id} 缺 CSS 色值`);
  }
});

test('"custom" 只有存在自定义配色时才生效，否则回落', () => {
  // 没有自定义配色 → 回落
  const bare = load();
  assert.equal(bare.paletteById(bare.CUSTOM_PALETTE_ID).id, bare.DEFAULT_PALETTE);

  // 有自定义配色 → 返回它
  const store = new Map<string, string>();
  store.set(
    "mini-custom-palette",
    JSON.stringify({
      id: "custom",
      label: "自定义",
      accent: [10, 20, 30],
      skin: "flat",
      barBg: "rgb(1, 2, 3)",
      barText: "#ffffff",
      barBgRgb: [1, 2, 3],
      barTextRgb: [255, 255, 255],
    }),
  );
  const withCustom = load(store);
  const got = withCustom.paletteById(withCustom.CUSTOM_PALETTE_ID);
  assert.equal(got.id, "custom");
  assert.deepEqual(JSON.parse(JSON.stringify(got.accent)), [10, 20, 30]);
});
