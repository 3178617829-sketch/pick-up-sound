<script setup lang="ts">
import Icon from "../../../app/components/Icon.vue";
import type { ApplicationSnapshot } from "../../../store/application-store.ts";
import type { PlaybackCallbacks } from "../../playback/root-playback.ts";
const props = defineProps<{ display: ApplicationSnapshot["playbackDisplay"]; callbacks: PlaybackCallbacks }>();
const cover = () => (props.display.presentation.thumbnailSet ? props.display.renderedTrack.thumbnailUrl : "");
const playing = () => props.display.buttonPlaying;
const hasTrack = () => props.display.currentTrack.hasCurrent;
</script>
<template>
  <section class="vinyl-stage" :class="{ 'is-playing': playing() }" aria-label="唱片机">
    <div class="vinyl-deck">
      <div class="vinyl-platter"></div>
      <div class="vinyl-disc">
        <div class="vinyl-grooves"></div>
        <div class="vinyl-label">
          <img v-if="cover()" :src="cover()" alt="" referrerpolicy="no-referrer" draggable="false" />
          <span v-else class="vinyl-label-empty">B</span>
        </div>
        <span class="vinyl-hole"></span>
        <span class="vinyl-shine"></span>
      </div>
      <div class="vinyl-arm">
        <span class="vinyl-arm-base"></span>
        <span class="vinyl-arm-rod"></span>
        <span class="vinyl-arm-head"></span>
      </div>
    </div>

    <div class="vinyl-meta">
      <p class="vinyl-kicker">{{ playing() ? "正在聆听" : hasTrack() ? "已暂停" : "等待播放" }}</p>
      <h2 class="vinyl-title">{{ display.renderedTrack.title }}</h2>
      <p class="vinyl-sub">
        <span class="vinyl-uploader">{{ display.renderedTrack.uploader }}</span>
        <span v-if="display.currentTrack.bvid" class="vinyl-bvid">{{ display.currentTrack.bvid }}</span>
      </p>
      <div class="vinyl-transport" aria-label="播放控制">
        <button class="vinyl-btn" type="button" aria-label="上一首" @click="callbacks.previous()">
          <Icon name="previous" />
        </button>
        <button class="vinyl-btn vinyl-btn-main" type="button" :aria-label="playing() ? '暂停' : '播放'" @click="callbacks.toggle()">
          <Icon :name="playing() ? 'pause' : 'play'" />
        </button>
        <button class="vinyl-btn" type="button" aria-label="下一首" @click="callbacks.next()">
          <Icon name="next" />
        </button>
      </div>
</div>
  </section>
</template>
<style scoped>
.vinyl-stage {
  --vinyl: clamp(280px, 30vw, 360px);
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6%;
  padding: 24px 32px 12px;
  overflow: hidden;
}

/* 盘座：一圈暗木色的托 */
.vinyl-deck {
  position: relative;
  width: var(--vinyl);
  height: var(--vinyl);
  flex: none;
}

.vinyl-platter {
  position: absolute;
  inset: -6%;
  border-radius: 50%;
  background: radial-gradient(circle at 50% 42%, rgba(212, 162, 83, .1), transparent 66%);
}

.vinyl-disc {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background:
    radial-gradient(circle at 50% 50%, #0b0908 0 26%, transparent 26.4%),
    repeating-radial-gradient(circle at 50% 50%, #171311 0 1px, #0e0b0a 1px 3px, #1b1614 3px 4px),
    radial-gradient(circle at 34% 26%, #241d19, #0d0a09 58%, #060505 100%);
  box-shadow: 0 26px 60px rgba(0, 0, 0, .62), 0 3px 0 rgba(255, 255, 255, .04) inset;
  animation: vinyl-spin 22s linear infinite;
  animation-play-state: paused;
}

.vinyl-stage.is-playing .vinyl-disc {
  animation-play-state: running;
}

@keyframes vinyl-spin {
  to { transform: rotate(360deg); }
}

.vinyl-grooves {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background: repeating-radial-gradient(circle at 50% 50%, rgba(255, 255, 255, .022) 0 1px, transparent 1px 5px);
  mask: radial-gradient(circle, transparent 30%, #000 32%, #000 96%, transparent 98%);
}

/* 盘心封面 */
.vinyl-label {
  position: absolute;
  inset: 32%;
  border-radius: 50%;
  overflow: hidden;
  background: linear-gradient(140deg, #e08a3c, #d94f5c 48%, #b8463f);
  box-shadow: 0 0 0 3px #0b0908, 0 2px 14px rgba(0, 0, 0, .5) inset;
}

.vinyl-label img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.vinyl-label-empty {
  display: grid;
  place-items: center;
  height: 100%;
  font-family: var(--font-serif);
  font-size: calc(var(--vinyl) * .14);
  color: rgba(255, 255, 255, .82);
}

.vinyl-hole {
  position: absolute;
  left: 50%;
  top: 50%;
  width: 8px;
  height: 8px;
  margin: -4px 0 0 -4px;
  border-radius: 50%;
  background: #060505;
  box-shadow: 0 0 0 2px rgba(255, 255, 255, .1);
}

.vinyl-shine {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background: conic-gradient(from 218deg, transparent 0 46deg, rgba(255, 255, 255, .055) 62deg 84deg, transparent 100deg 236deg, rgba(255, 255, 255, .035) 254deg 272deg, transparent 292deg);
  pointer-events: none;
}

/* 唱臂 */
.vinyl-arm {
  position: absolute;
  right: -14%;
  top: -10%;
  width: 62%;
  height: 62%;
  transform-origin: 90% 8%;
  transform: rotate(20deg);
  transition: transform .85s cubic-bezier(.22, .8, .24, 1);
  pointer-events: none;
}

.vinyl-stage.is-playing .vinyl-arm {
  transform: rotate(32deg);
}

.vinyl-arm-base {
  position: absolute;
  right: 0;
  top: 2%;
  width: 26%;
  height: 26%;
  border-radius: 50%;
  background: radial-gradient(circle at 36% 30%, #86603a, #3d2b1c 62%, #241a11);
  box-shadow: 0 8px 18px rgba(0, 0, 0, .55), 0 1px 0 rgba(255, 214, 160, .18) inset;
}

.vinyl-arm-rod {
  position: absolute;
  right: 10%;
  top: 12%;
  width: 74%;
  height: 3px;
  border-radius: 2px;
  background: linear-gradient(90deg, #6a4a2c, #c69a5e 40%, #e6c088 70%, #8a6437);
  transform-origin: right center;
  transform: rotate(-34deg);
  box-shadow: 0 2px 6px rgba(0, 0, 0, .45);
}

.vinyl-arm-head {
  position: absolute;
  right: 64%;
  top: 44%;
  width: 13%;
  height: 9%;
  border-radius: 2px;
  background: linear-gradient(160deg, #d8b784, #8a6437 70%, #5a4024);
  transform: rotate(-34deg);
  box-shadow: 0 3px 8px rgba(0, 0, 0, .5);
}

/* 文案 */
.vinyl-meta {
  min-width: 0;
  max-width: 380px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.vinyl-kicker {
  margin: 0;
  font-size: 11px;
  letter-spacing: .34em;
  color: var(--text3);
  text-transform: uppercase;
}

.vinyl-title {
  margin: 0;
  font-family: var(--font-serif);
  font-weight: 500;
  font-size: clamp(20px, 2.4vw, 30px);
  line-height: 1.32;
  color: var(--text);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.vinyl-sub {
  margin: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  font-size: 12px;
  color: var(--text2);
}

.vinyl-bvid {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text3);
}

.vinyl-transport {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 6px;
}

.vinyl-btn {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  border: 1px solid var(--line2);
  background: transparent;
  color: var(--text2);
  cursor: pointer;
  transition: color .15s, border-color .15s, transform .15s;
}

.vinyl-btn:hover {
  color: var(--accent);
  border-color: var(--accent);
}

.vinyl-btn:active {
  transform: scale(.94);
}

.vinyl-btn-main {
  width: 52px;
  height: 52px;
  border: none;
  background: var(--accent);
  color: var(--play-fg);
}

.vinyl-btn-main:hover {
  color: var(--play-fg);
  filter: brightness(1.08);
}

@media (max-width: 900px) {
  .vinyl-stage { flex-direction: column; gap: 20px; padding: 16px; }
  .vinyl-meta { max-width: none; text-align: center; align-items: center; }
  .vinyl-sub { justify-content: center; }
}
</style>
