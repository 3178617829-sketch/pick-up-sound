/*
 * storage.ts 门面契约测试。
 *
 * 这个文件是所有 localStorage 读写的唯一入口，它的两条性质必须钉住：
 *   1. **key 的值不许改** —— 改了等于把老用户的设置清空，而且不报错、测不出来；
 *   2. **一律不抛** —— 隐私模式 / 配额满 / 受限 WebView 里 localStorage 本身会抛，
 *      读写原语必须把它咽掉，否则异常会冲出 UI 线程。
 *
 * 不引第三方测试框架：项目已有 node:test + VM 切片这一套，跟其它测试保持一致。
 */
import assert from "node:assert/strict";
import path from "node:path";
import { test } from "node:test";
import vm from "node:vm";

import { readFileSync } from "./helpers/module-syntax.ts";
import { sourceSlice } from "./helpers/source-slice.ts";

const source = readFileSync(path.join(import.meta.dirname, "../ui/shared/storage.ts"), "utf8");

/** 整份装进 VM；末尾追加一个对象字面量把导出名捞出来。 */
const EXPORTS = [
  "THEME_KEY",
  "ACCENT_COLOR_KEY",
  "VOLUME_KEY",
  "LOUDNESS_KEY",
  "SIDEBAR_KEY",
  "TRACK_COLUMNS_KEY",
  "LAST_SEARCH_KEY",
  "SHUFFLE_ORDER_KEY",
  "SHUFFLE_LIMIT_KEY",
  "PALETTE_KEY",
  "CUSTOM_PALETTE_KEY",
  "FOLLOW_SONG_KEY",
  "SURFACE_MODE_KEY",
  "SKIN_KEY",
  "MINI_SCALE_KEY",
  "MINI_POSITION_KEY",
  "readString",
  "writeString",
  "removeKey",
  "readJson",
  "writeJson",
].join(", ");

interface Facade {
  readString(key: string): string | null;
  writeString(key: string, value: string): boolean;
  removeKey(key: string): void;
  readJson<T>(key: string): T | null;
  writeJson(key: string, value: unknown): boolean;
  [key: string]: unknown;
}

interface Behaviour {
  throwOnRead?: boolean;
  throwOnWrite?: boolean;
  throwOnRemove?: boolean;
}

function load(behaviour: Behaviour = {}): { facade: Facade; store: Map<string, string> } {
  const store = new Map<string, string>();
  const localStorage = {
    getItem(key: string) {
      if (behaviour.throwOnRead) throw new Error("storage denied");
      return store.has(key) ? store.get(key)! : null;
    },
    setItem(key: string, value: string) {
      if (behaviour.throwOnWrite) throw new Error("quota exceeded");
      store.set(key, String(value));
    },
    removeItem(key: string) {
      if (behaviour.throwOnRemove) throw new Error("storage denied");
      store.delete(key);
    },
  };
  const code = sourceSlice(source, "ui/shared/storage.ts", null, null);
  const context = vm.createContext({ localStorage });
  const facade = new vm.Script(`${code}\n;({${EXPORTS}})`).runInContext(context) as Facade;
  return { facade, store };
}

test("key 的值与历史版本保持一致（改值等于清空老用户设置）", () => {
  const { facade } = load();
  assert.deepEqual(
    {
      THEME_KEY: facade.THEME_KEY,
      ACCENT_COLOR_KEY: facade.ACCENT_COLOR_KEY,
      VOLUME_KEY: facade.VOLUME_KEY,
      LOUDNESS_KEY: facade.LOUDNESS_KEY,
      SIDEBAR_KEY: facade.SIDEBAR_KEY,
      TRACK_COLUMNS_KEY: facade.TRACK_COLUMNS_KEY,
      LAST_SEARCH_KEY: facade.LAST_SEARCH_KEY,
      SHUFFLE_ORDER_KEY: facade.SHUFFLE_ORDER_KEY,
      SHUFFLE_LIMIT_KEY: facade.SHUFFLE_LIMIT_KEY,
      PALETTE_KEY: facade.PALETTE_KEY,
      CUSTOM_PALETTE_KEY: facade.CUSTOM_PALETTE_KEY,
      FOLLOW_SONG_KEY: facade.FOLLOW_SONG_KEY,
      SURFACE_MODE_KEY: facade.SURFACE_MODE_KEY,
      SKIN_KEY: facade.SKIN_KEY,
      MINI_SCALE_KEY: facade.MINI_SCALE_KEY,
      MINI_POSITION_KEY: facade.MINI_POSITION_KEY,
    },
    {
      THEME_KEY: "bilibili-music.theme",
      ACCENT_COLOR_KEY: "bilibili-music.accent-color",
      VOLUME_KEY: "bilibili-music.volume",
      LOUDNESS_KEY: "bilibili-music.loudness-normalization",
      SIDEBAR_KEY: "bilibili-music.sidebar",
      TRACK_COLUMNS_KEY: "bilibili-music.track-columns",
      LAST_SEARCH_KEY: "bilibili-music.last-search",
      SHUFFLE_ORDER_KEY: "bilibili-music.shuffle-collection-order",
      SHUFFLE_LIMIT_KEY: "bilibili-music.shuffle-collection-limit",
      PALETTE_KEY: "mini-palette",
      CUSTOM_PALETTE_KEY: "mini-custom-palette",
      FOLLOW_SONG_KEY: "mini-follow-song",
      SURFACE_MODE_KEY: "mini-surface-mode",
      SKIN_KEY: "mini-skin",
      MINI_SCALE_KEY: "mini-scale",
      MINI_POSITION_KEY: "bilibili-music.mini-player-position",
    },
  );
});

test("字符串读写往返，没写过的 key 读到 null", () => {
  const { facade, store } = load();
  assert.equal(facade.readString(facade.THEME_KEY as string), null);
  assert.equal(facade.writeString(facade.THEME_KEY as string, "dark"), true);
  assert.equal(facade.readString(facade.THEME_KEY as string), "dark");
  assert.equal(store.get("bilibili-music.theme"), "dark");
});

test("readJson 对没值、空串、坏 JSON 一律返回 null", () => {
  const { facade, store } = load();
  const key = facade.SIDEBAR_KEY as string;
  assert.equal(facade.readJson(key), null);
  store.set(key, "");
  assert.equal(facade.readJson(key), null);
  store.set(key, "{not json");
  assert.equal(facade.readJson(key), null);
});

test("JSON 读写往返保留结构", () => {
  const { facade } = load();
  const key = facade.TRACK_COLUMNS_KEY as string;
  const value = { title: 240, artist: 180 };
  assert.equal(facade.writeJson(key, value), true);
  // VM 里 JSON.parse 出来的对象原型属另一个 realm，deepStrictEqual 会判
  // "same structure but not reference-equal"，所以按序列化结果比。
  assert.equal(JSON.stringify(facade.readJson(key)), JSON.stringify(value));
});

test("localStorage 抛异常时读写不崩：读到 null、写返回 false、删静默", () => {
  const reading = load({ throwOnRead: true }).facade;
  assert.equal(reading.readString("anything"), null);
  assert.equal(reading.readJson("anything"), null);

  const writing = load({ throwOnWrite: true }).facade;
  assert.equal(writing.writeString("anything", "x"), false);
  assert.equal(writing.writeJson("anything", { a: 1 }), false);

  const removing = load({ throwOnRemove: true }).facade;
  assert.doesNotThrow(() => removing.removeKey("anything"));
});
