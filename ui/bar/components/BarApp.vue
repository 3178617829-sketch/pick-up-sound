<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { paletteById, fixAccent, isLightPalette } from "../../shared/palettes.ts";
import { PALETTE_KEY, readString } from "../../shared/storage.ts";
import { barBridge, barInvoke, barListen } from "../bar-bridge.ts";

type BarState = { title: string; uploader: string; lyric: string; playing: boolean; progress: number };

const title = ref("尚未播放");
const uploader = ref("");
const lyric = ref("");
const playing = ref(false);
const progress = ref(0);

function applyPalette(): void {
  const p = paletteById(readString(PALETTE_KEY));
  const accent = fixAccent(p);
  const light = isLightPalette(p);
  const root = document.documentElement;
  root.style.setProperty("--bar-bg", p.barBg);
  root.style.setProperty("--bar-text", p.barText);
  root.style.setProperty("--bar-accent", `rgb(${accent[0]}, ${accent[1]}, ${accent[2]})`);
  root.style.setProperty("--bar-muted", light ? "rgba(43, 38, 34, 0.52)" : "rgba(246, 239, 230, 0.52)");
}
applyPalette();
addEventListener("storage", (e) => { if (e.key === PALETTE_KEY) applyPalette(); });
// bar-palette-poll：storage 事件在多个 WebView 之间不一定触发，主动轮询兜底
let lastPalette = readString(PALETTE_KEY);
setInterval(() => {
  const now = readString(PALETTE_KEY);
  if (now !== lastPalette) { lastPalette = now; applyPalette(); }
}, 1200);

function apply(payload: Partial<BarState> | undefined) {
  if (!payload) return;
  if (typeof payload.title === "string") title.value = payload.title;
  if (typeof payload.uploader === "string") uploader.value = payload.uploader;
  if (typeof payload.lyric === "string") lyric.value = payload.lyric;
  if (typeof payload.playing === "boolean") playing.value = payload.playing;
  if (typeof payload.progress === "number") progress.value = payload.progress;
}

// 主窗口被关掉后，点这里把它叫回来
function openMain() {
  void barInvoke("show_main_window_cmd");
}

function control(action: "previous" | "play_pause" | "next") {
  void barInvoke("bar_control", { action });
}

let unlisten: (() => void) | null = null;

onMounted(() => {
  if (!barBridge()?.event) return;
  void barListen<BarState>("bar-state", payload => apply(payload)).then(off => {
    unlisten = off;
  });
  void barInvoke("taskbar_bar_ready");
});

onBeforeUnmount(() => {
  unlisten?.();
});
</script>

<template>
  <div class="bar" :data-playing="playing">
    <div class="bar-thumb" title="打开主窗口" @click="openMain">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M10 19V6.6l8-1.6v10.4" /><circle cx="7.6" cy="19.2" r="2.6" /><circle cx="15.6" cy="17.4" r="2.6" /></svg>
    </div>
    <div class="bar-body">
      <p class="bar-title">{{ title }}</p>
      <p class="bar-lyric" :class="{ 'is-empty': !lyric }">{{ lyric || uploader || "—" }}</p>
    </div>
    <div class="bar-ctl">
      <button type="button" class="bar-btn" title="上一首" @click="control('previous')">
        <svg viewBox="0 0 24 24"><path d="M18 6v12L9 12l9-6Z" /><rect x="5" y="6" width="2.4" height="12" rx="1.2" /></svg>
      </button>
      <button type="button" class="bar-btn is-main" :title="playing ? '暂停' : '播放'" @click="control('play_pause')">
        <svg v-if="playing" viewBox="0 0 24 24"><rect x="7" y="5" width="3.6" height="14" rx="1.6" /><rect x="13.4" y="5" width="3.6" height="14" rx="1.6" /></svg>
        <svg v-else viewBox="0 0 24 24"><path d="M8 5.4v13.2c0 .9 1 1.5 1.8 1L19.4 13c.8-.5.8-1.6 0-2.1L9.8 4.4C9 3.9 8 4.5 8 5.4Z" /></svg>
      </button>
      <button type="button" class="bar-btn" title="下一首" @click="control('next')">
        <svg viewBox="0 0 24 24"><path d="M6 6v12l9-6-9-6Z" /><rect x="16.6" y="6" width="2.4" height="12" rx="1.2" /></svg>
      </button>
    </div>
    <div class="bar-track"><span :style="{ width: `${Math.round(progress * 100)}%` }" /></div>
  </div>
</template>