<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import DynamicBackground from "../../features/settings/components/DynamicBackground.vue";
import { paletteById, paletteVars, PALETTES, DEFAULT_PALETTE } from "../../shared/palettes.ts";
import { MINI_SCALE_KEY, PALETTE_KEY, readString, writeString } from "../../shared/storage.ts";
import { DEFAULT_BACKGROUND_DIM } from "../../features/settings/settings-state.ts";
import { miniDelay, miniEvery, miniOff, miniOn, miniSetScale, miniStartResize, miniStop, miniViewport } from "../mini-bridge.ts";
import type {MiniView,MiniCallbacks} from "../view-model.ts";
defineProps<{view:MiniView;callbacks:MiniCallbacks}>();

const SCALES = [0.8, 1, 1.2];

/** 配色：40 套 palette 共用同一份定义，右键循环切换 */
const paletteId = ref<string>(readString(PALETTE_KEY) || DEFAULT_PALETTE);
const palette = computed(() => paletteById(paletteId.value));
const paletteStyle = computed(() => paletteVars(palette.value));
const scale = ref<number>(Number(readString(MINI_SCALE_KEY) || "1"));

// 与 `src-tauri/src/mini_player.rs` 的 `MINI_WINDOW_WIDTH`/`MINI_WINDOW_HEIGHT`
// 是同一组数字：这边用来算「内容铺满窗口」的缩放比，那边用来定窗口初始尺寸。
const BASE_WIDTH = 480;
const BASE_HEIGHT = 224;

/** 让内容等比铺满窗口：取宽高里更紧的那一边，保证永远不裁切 */
function syncZoom() {
  const { width, height } = miniViewport();
  const byWidth = width / BASE_WIDTH;
  const byHeight = height / BASE_HEIGHT;
  const z = Math.min(2.4, Math.max(0.5, Math.min(byWidth, byHeight)));
  document.documentElement.style.zoom = String(Math.round(z * 1000) / 1000);
}

function cycleSkin() {
  const index = PALETTES.findIndex((item) => item.id === paletteId.value);
  paletteId.value = PALETTES[(index + 1) % PALETTES.length].id;
  writeString(PALETTE_KEY, paletteId.value);
  const notice = document.getElementById("mini-toast");
  if (notice) {
    notice.textContent = `配色：${palette.value.label}`;
    notice.hidden = false;
    miniDelay(() => { notice.hidden = true; }, 1400);
  }
}

function applyScale(next: number) {
  const value = Math.min(1.6, Math.max(0.6, Math.round(next * 20) / 20));
  scale.value = value;
  writeString(MINI_SCALE_KEY, String(value));
  syncZoom();
  miniSetScale(value);
}

function onWheel(event: WheelEvent) {
  if (!event.ctrlKey) return;
  event.preventDefault();
  applyScale(scale.value + (event.deltaY > 0 ? -0.05 : 0.05));
}

/** 按住边或角：把缩放交给系统，最跟手 */
function beginResize(direction: string, event: PointerEvent) {
  event.preventDefault();
  event.stopPropagation();
  miniStartResize(direction);
}

/** 双击空白处：小 / 中 / 大三档轮换 */
function cycleScale() {
  const next = SCALES.find((item) => item > scale.value + 0.02) ?? SCALES[0];
  applyScale(next);
}

/** 窗口尺寸一变（系统拖拽或代码改动），内容立刻等比铺满 */
function onResize() {
  syncZoom();
}

/** 主窗口设置面板改了配色，这里立刻跟上 */
function onStorage(event: StorageEvent) {
  if (event.key === PALETTE_KEY && event.newValue) paletteId.value = event.newValue;
  if (event.key === MINI_SCALE_KEY && event.newValue) {
    const value = Number(event.newValue);
    if (Number.isFinite(value)) applyScale(value);
  }
}

let palettePoll = 0;

onMounted(() => {
  applyScale(scale.value);
  miniDelay(() => syncZoom(), 60);
  // storage 事件在多个 WebView 之间不一定触发，主动轮询兜底（只建一个，卸载时清掉）
  palettePoll = miniEvery(() => {
    const now = readString(PALETTE_KEY);
    if (now && now !== paletteId.value) paletteId.value = now;
  }, 1200);
  miniOn("wheel", onWheel as (event: Event) => void, { passive: false });
  miniOn("resize", onResize);
  miniOn("storage", onStorage as (event: Event) => void);
});

onUnmounted(() => {
  miniStop(palettePoll);
  miniOff("wheel", onWheel as (event: Event) => void);
  miniOff("resize", onResize);
  miniOff("storage", onStorage as (event: Event) => void);
});
</script>
<template>
<main id="mini-drag-region" class="mini-shell" :data-image-scheme="view.snapshot.theme === 'image' ? view.snapshot.imageScheme ?? 'dark' : undefined" :data-image-mode="view.snapshot.theme === 'image' ? ((view.snapshot.backgroundDim ?? DEFAULT_BACKGROUND_DIM) < 40 ? 'adaptive' : 'uniform') : undefined" :style="{'--mini-background':view.backgroundUrl ? `url(&quot;${view.backgroundUrl}&quot;)` : undefined,'--background-dim':String((view.snapshot.backgroundDim ?? DEFAULT_BACKGROUND_DIM) / 100),'--dynamic-color-1':view.snapshot.dynamicColors?.[0],'--dynamic-color-2':view.snapshot.dynamicColors?.[1],'--dynamic-color-3':view.snapshot.dynamicColors?.[2]}" @pointerdown="callbacks.drag" @contextmenu.prevent="cycleSkin" aria-label="迷你播放器">
      <DynamicBackground v-if="view.snapshot.theme === 'dynamic' && view.background" :model="view.background" />

      <div class="mini-panel" data-layout="card" :data-skin="palette.skin" :style="paletteStyle" :data-playing="view.hasSnapshot ? String(Boolean(view.snapshot.isPlaying)) : 'false'" @dblclick.self="cycleScale">
        <div class="mini-copy">
          <div id="mini-title-viewport" class="mini-title-viewport" :class="{'has-scrolling-title': view.scrolling}">
            <strong id="mini-title" class="mini-title" :class="{'is-scrolling': view.scrolling}" :title="view.hasSnapshot ? view.snapshot.title || '尚未播放' : undefined" :style="{'--mini-title-distance':view.distance || undefined,'--mini-title-duration':view.duration || undefined}">{{ view.snapshot.title || "尚未播放" }}</strong>
          </div>
          <div id="mini-uploader" class="mini-uploader">{{ view.snapshot.uploader || "" }}</div>
          <div class="mini-controls" aria-label="播放控制">
            <button id="mini-previous" @click="callbacks.command('previous')" class="mini-icon-button" type="button" aria-label="上一首" title="上一首" :disabled="!view.snapshot.canPrevious">
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 5h2v14H6Zm3 7 9-7v14Z" /></svg>
            </button>
            <button id="mini-play-pause" @click="callbacks.command('toggle_play')" class="mini-icon-button mini-play-button" type="button" :aria-label="view.snapshot.isPlaying ? '暂停' : '播放'" title="播放/暂停" :disabled="!view.snapshot.hasCurrent">
              <svg class="mini-icon-play" viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.6v12.8a1 1 0 0 0 1.53.85l9.6-6.4a1 1 0 0 0 0-1.7l-9.6-6.4A1 1 0 0 0 8 5.6Z" /></svg>
              <svg class="mini-icon-pause" viewBox="0 0 24 24" aria-hidden="true"><path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" /></svg>
            </button>
            <button id="mini-next" @click="callbacks.command('next')" class="mini-icon-button" type="button" aria-label="下一首" title="下一首" :disabled="!view.snapshot.canNext">
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M16 5h2v14h-2ZM15 12 6 5v14Z" /></svg>
            </button>
            <button id="mini-favorite" @click="callbacks.command('toggle_favorite')" class="mini-icon-button mini-favorite" :class="{'is-favorited': Boolean(view.snapshot.isFavorited)}" type="button" :aria-label="view.snapshot.isFavorited ? '取消收藏' : '收藏'" title="收藏" :disabled="!view.snapshot.hasCurrent">
              <svg class="mini-icon-heart" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 21.2 10.7 20C5.4 15.2 2 12.2 2 8.4A5.4 5.4 0 0 1 7.5 3c1.8 0 3.5.8 4.5 2.1A5.6 5.6 0 0 1 16.5 3 5.4 5.4 0 0 1 22 8.4c0 3.8-3.4 6.8-8.7 11.6Z" /></svg>
            </button>
          </div>
        </div>

        <div class="mini-vinyl">
          <img id="mini-cover" class="mini-cover" alt="" :src="view.snapshot.thumbnailUrl || undefined" :hidden="!view.snapshot.thumbnailUrl" referrerpolicy="no-referrer" />
          <span v-if="!view.snapshot.thumbnailUrl" class="mini-vinyl-mark">B</span>
          <span class="mini-vinyl-hole"></span>
        </div>

        <button id="mini-restore" @click="callbacks.restore" class="mini-icon-button mini-restore" type="button" aria-label="还原主窗口" title="还原主窗口">
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M8 4H4v4h2V7.4l3.3 3.3 1.4-1.4L7.4 6H8Zm4 12h4v-4h-2v.6l-3.3-3.3-1.4 1.4 3.3 3.3H12Z" /></svg>
        </button>
      </div>

      <p id="mini-notice" role="status" aria-live="polite" :hidden="!view.snapshot.notice">{{ view.snapshot.notice || "" }}</p>
      <button id="mini-notice-more" class="mini-notice-more" type="button" aria-controls="mini-notice-full" @click="callbacks.more" :aria-expanded="view.expanded" :hidden="!view.snapshot.notice">全文</button>
      <div id="mini-notice-full" class="mini-notice-full" role="region" aria-label="提醒全文" tabindex="0" @keydown="callbacks.noticeKey" :hidden="!view.expanded">{{ view.expanded ? view.snapshot.notice || "" : "" }}</div>

      <i class="mini-zone mini-zone-n" @pointerdown="beginResize('North', $event)"></i>
      <i class="mini-zone mini-zone-s" @pointerdown="beginResize('South', $event)"></i>
      <i class="mini-zone mini-zone-e" @pointerdown="beginResize('East', $event)"></i>
      <i class="mini-zone mini-zone-w" @pointerdown="beginResize('West', $event)"></i>
      <i class="mini-zone mini-zone-ne" @pointerdown="beginResize('NorthEast', $event)"></i>
      <i class="mini-zone mini-zone-nw" @pointerdown="beginResize('NorthWest', $event)"></i>
      <i class="mini-zone mini-zone-se" @pointerdown="beginResize('SouthEast', $event)"></i>
      <i class="mini-zone mini-zone-sw" @pointerdown="beginResize('SouthWest', $event)"></i>
      <i class="mini-grip" title="拖动缩放，双击循环三档，右键换配色" @pointerdown="beginResize('SouthEast', $event)" @dblclick.stop.prevent="cycleScale()"></i>
      <p id="mini-toast" class="mini-toast" role="status" aria-live="polite" hidden></p>
    </main>
</template>
