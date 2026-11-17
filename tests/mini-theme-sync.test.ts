import assert from "node:assert/strict";
import { test } from "node:test";
import { createMiniHostFixture } from "./helpers/mini-host-fixture.ts";
import { setLivePalette } from "../ui/shared/live-palette.ts";
import { DEFAULT_PALETTE, fixAccent, paletteById, solidOf } from "../ui/shared/palettes.ts";

// 小组件的强调色与底色现在只认 live 配色（palette 的精确色），不再由六个色名反推；
// 所以「重推」的触发源是 store 里的主题/背景字段 + live 配色变化，不是 accentColor。
function accentOf(id: string) {
  const [r, g, b] = fixAccent(paletteById(id));
  return { r, g, b };
}

test("mini theme notifications pass no arguments and select only changed projected values", async () => {
  const h = await createMiniHostFixture(), sizes: number[] = [];
  const unlisten = h.context.subscribeMiniPlayerTheme((...args: unknown[]) => sizes.push(args.length));
  h.store.actions.settings.setSlider("alpha", 55, false);
  h.store.actions.settings.panel(true);
  h.store.actions.playbackSettings.setUserVolume(0.5);
  h.store.actions.settings.setTheme("dark", false);
  h.store.actions.settings.setAccent("pink", "dark", false);
  assert.deepEqual(sizes, []);
  // 明暗现在由 live 配色决定：换到浅底配色才算「主题有变化」，必须重推
  setLivePalette(paletteById("paper"));
  // 毛玻璃值也在小组件 payload 里，拖滑杆必须重推才能实时预览
  h.store.actions.settings.setSlider("blur", 44, false);
  assert.deepEqual(sizes, [0, 0]);
  h.store.actions.settings.setSlider("blur", 44, false);
  assert.equal(sizes.length, 2);
  unlisten(); h.store.actions.settings.setTheme("dark", false);
  assert.equal(sizes.length, 2);
});

test("mini theme changes coalesce within each frame and publish final values in a complete payload", async () => {
  const h = await createMiniHostFixture(); await h.fire("mini-player-ready");
  setLivePalette(paletteById("navy"));
  setLivePalette(paletteById("moss"));
  assert.equal(h.emissions.length, 1); assert.equal(h.frameCount(), 1);
  await h.flushFrame();
  assert.equal(h.emissions.length, 2);
  assert.equal(h.emissions[1].payload.theme, "dark");
  assert.deepEqual(h.emissions[1].payload.accent, accentOf("moss"));
  assert.equal(h.emissions[1].payload.paletteBg, solidOf(paletteById("moss").barBg));
  assert.deepEqual(Object.keys(h.emissions[1].payload).sort(), Object.keys(h.emissions[0].payload).sort());
  setLivePalette(paletteById("purple"));
  setLivePalette(paletteById("orange"));
  assert.equal(h.emissions.length, 2); await h.flushFrame();
  assert.equal(h.emissions.length, 3);
  assert.deepEqual(h.emissions[2].payload.accent, accentOf("orange"));
  setLivePalette(paletteById(DEFAULT_PALETTE));
});

test("mini theme synchronization ignores other settings and unchanged projected values", async () => {
  const h = await createMiniHostFixture(); await h.fire("mini-player-ready");
  h.store.actions.settings.setSlider("alpha", 80, false);
  h.store.actions.settings.setSlider("dim", 70, false);
  h.store.actions.settings.setAiFormat("openai");
  h.store.actions.settings.setTheme("dark", false);
  h.store.actions.settings.setAccent("pink", "dark", false);
  await h.flush(); assert.equal(h.emissions.length, 1); assert.equal(h.frameCount(), 0);
});

test("mini theme synchronization waits for ready and later changes publish", async () => {
  const h = await createMiniHostFixture();
  // 手选浅底配色时设置面板会同时同步明暗轴（applyTheme(light)），这里照做
  setLivePalette(paletteById("paper"));
  h.store.actions.settings.setTheme("light", false);
  assert.equal(h.frameCount(), 0); await h.flush(); assert.equal(h.emissions.length, 0);
  await h.fire("mini-player-ready");
  assert.equal(h.emissions.length, 1); assert.equal(h.emissions[0].payload.theme, "light");
  setLivePalette(paletteById("navy")); await h.flush();
  assert.equal(h.emissions.length, 2); assert.deepEqual(h.emissions[1].payload.accent, accentOf("navy"));
  setLivePalette(paletteById(DEFAULT_PALETTE));
});

test("mini close cancels theme subscription and pending frame and reopening subscribes again", async () => {
  const h = await createMiniHostFixture(); await h.fire("mini-player-ready");
  const target = h.listenTargets.find(item => item.name === "tauri://destroyed")?.target;
  assert.equal(target?.kind, "Window"); assert.equal(target?.label, "mini");
  assert.equal(h.themeSubscriberCount(), 1);
  h.store.actions.settings.setTheme("light", false); assert.equal(h.frameCount(), 1);
  // tauri-2.11.3/src/manager/window.rs:177: destroyed emits unit (JSON null); handler does not inspect payload.
  await h.fire("tauri://destroyed");
  assert.equal(h.themeSubscriberCount(), 0); assert.equal(h.frameCount(), 0);
  setLivePalette(paletteById("moss")); await h.flush();
  assert.equal(h.emissions.length, 1);
  await h.fire("mini-player-ready"); assert.equal(h.themeSubscriberCount(), 1);
  setLivePalette(paletteById("purple")); await h.flush();
  assert.equal(h.emissions.length, 3);
  setLivePalette(paletteById("orange")); await h.flush();
  assert.equal(h.emissions.length, 4); assert.deepEqual(h.emissions[3].payload.accent, accentOf("orange"));
  setLivePalette(paletteById(DEFAULT_PALETTE));
});

test("mini beforeunload cancels theme subscription and pending frame", async () => {
  const h = await createMiniHostFixture(); await h.fire("mini-player-ready");
  setLivePalette(paletteById("paper"));
  await h.publishEvent("beforeunload");
  assert.equal(h.themeSubscriberCount(), 0); assert.equal(h.frameCount(), 0);
  setLivePalette(paletteById("navy")); await h.flush();
  assert.equal(h.emissions.length, 1);
  setLivePalette(paletteById(DEFAULT_PALETTE));
});

test("mini theme frame reads latest full non-sensitive payload and leaves existing publish events immediate", async () => {
  const h = await createMiniHostFixture(); h.setTrack(); await h.fire("mini-player-ready");
  setLivePalette(paletteById("paper"));
  h.store.actions.playbackRequest.activateAudio(1, "private-theme-audio-sentinel", 1);
  h.node("#status").textContent = "最新提示";
  await h.publishEvent("bilibili-music-notice-change");
  assert.equal(h.emissions.length, 2);
  await h.flushFrame(); assert.equal(h.emissions.length, 3);
  assert.equal(h.emissions[2].payload.status, "最新提示");
  assert.equal(JSON.stringify(h.emissions[2].payload).includes("private-theme-audio-sentinel"), false);
});
