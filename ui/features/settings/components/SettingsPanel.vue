<script setup lang="ts">
import FloatingBackdrop from "./FloatingBackdrop.vue";
import type {DynamicBackgroundPresentation} from "../dynamic-background.ts";
import {ref,computed} from "vue";
import type {ComponentPublicInstance} from "vue";
import type {ObjectDirective} from 'vue';
import type {draftBindings} from "../../../platform/draft-ports.ts";
import {applySettingsField} from './field-directive.ts';
import type {SettingsControl,SettingsControls} from '../settings-controls.ts';
import type {SettingsSnapshot} from '../settings-state.ts';
import {DEFAULT_BACKGROUND_DIM,normalizeThemeSource} from '../settings-state.ts';
import type {ThemeSource} from '../settings-state.ts';
import type {SettingsCallbacks} from '../root-settings.ts';
const props=defineProps<{draftBindings:typeof draftBindings;surfaceVisible?:boolean;background?:DynamicBackgroundPresentation;onGeometry?:(element:HTMLElement)=>()=>void;appearanceStatus:string;purgeBusy:boolean;state:SettingsSnapshot;model:SettingsControls;callbacks:SettingsCallbacks;normalization:boolean;normalizationVersion:number}>();
const empty:SettingsControl={};
const field=(name:string)=>props.model[name]??empty;
const vField:ObjectDirective<HTMLInputElement|HTMLSelectElement,SettingsControl>={mounted:(element,binding)=>applySettingsField(element,binding.value),updated:(element,binding)=>applySettingsField(element,binding.value,binding.oldValue ?? undefined)};
function draftRef(value:Element|ComponentPublicInstance|null):void {
 const input=value as HTMLInputElement|null;
 props.draftBindings.key(input ? {read:()=>input.value,write:value=>{input.value=value;}} : null);
}
function aiDetailsRef(value:Element|ComponentPublicInstance|null):void {
 const details=value as HTMLDetailsElement|null;
 props.draftBindings.aiDetails(details ? ()=>{details.open=true;} : null);
}
import {PALETTES,PALETTE_KEY,FOLLOW_SONG_KEY,DEFAULT_PALETTE,paletteById,readCustomPalette,activateCustomPalette} from "../../../shared/palettes.ts";
import {paletteFromHue,rgbToHsl} from "../../../shared/custom-palette.ts";
import { SKIN_KEY, BACKGROUND_MODE_KEY, readString, writeString } from "../../../shared/storage.ts";
import {emitPaletteChange,emitFollowSongChange} from "../settings-events.ts";
const paletteId = ref<string>(readString(PALETTE_KEY) || DEFAULT_PALETTE);
// 跟随歌曲只认 "1"，与 ui/shared/follow-song.ts 的判定保持一致
const followSong = ref<boolean>(readString(FOLLOW_SONG_KEY) === "1");
// 外观「主题」五选一：状态层是唯一事实来源，这里只读来画选中态
const themeSource = computed<ThemeSource>(() => normalizeThemeSource(props.state.themeSource));
// 自定义配色：40 套预设施不开的那一格。存量值来自图片取色，也可以在这里手动调。
const customPalette = ref(readCustomPalette());
const colorOpen = ref(false);
const colorTab = ref<"pick"|"custom">("pick");
const customHue = ref(0);
const customSat = ref(42);
const customSwatch = computed(() => {
  const palette = paletteFromHue(customHue.value, customSat.value / 100);
  return {
    bar: String(palette.barBg),
    accent: `rgb(${palette.accent[0]}, ${palette.accent[1]}, ${palette.accent[2]})`,
    text: String(palette.barText),
  };
});
function openColor():void {
  colorOpen.value = true;
  colorTab.value = "pick";
}
function openCustom():void {
  const current = customPalette.value;
  if (current) {
    const { h, s } = rgbToHsl(current.accent[0], current.accent[1], current.accent[2]);
    customHue.value = Math.round(h);
    customSat.value = Math.max(5, Math.min(90, Math.round(s * 100)));
  }
  colorOpen.value = true;
  colorTab.value = "custom";
}
function applyCustom():void {
  const palette = paletteFromHue(customHue.value, customSat.value / 100);
  activateCustomPalette(palette);
  customPalette.value = palette;
  paletteId.value = "custom";
  writeString(SKIN_KEY, palette.skin);
  // 自己调的颜色不该再被「跟随封面」每首歌覆盖回去
  if (followSong.value) {
    followSong.value = false;
    writeString(FOLLOW_SONG_KEY, "0");
    emitFollowSongChange(false);
  }
  // 「选择颜色」这一档：状态层按配色亮度把明暗轴一起写回，这里不再手动补 theme
  props.callbacks.themeSource("picked");
  emitPaletteChange("custom");
  colorOpen.value = false;
}
function customFromImage():void {
  colorOpen.value = false;
  props.callbacks.chooseBackground();
}
function previewOf(id:string):string {
 // 预览色块直接用配色表自己的底色，不再维护第二份色值表
 return paletteById(id).barBg;
}
function setPalette(id:string):void {
 const p = paletteById(id);
 paletteId.value = p.id;
 writeString(PALETTE_KEY, p.id);
 writeString(SKIN_KEY, p.skin);
 // 手选了一套 = 不要跟随封面，否则下一秒就被封面覆盖掉
 if (followSong.value) {
  followSong.value = false;
  writeString(FOLLOW_SONG_KEY, "0");
  emitFollowSongChange(false);
 }
 // 三处（主窗 / 小组件 / 任务栏条）共用同一个 live 色源，状态层连明暗轴一起写回
 props.callbacks.themeSource("picked");
 emitPaletteChange(p.id);
}
/** 从图片 / 跟随歌曲退回时落在哪一档：沿用当前这套配色的明暗 */
function fallbackSource():ThemeSource {
 return props.state.theme === "light" ? "light" : "dark";
}
/** 五选一唯一入口：黑/白是两套现成预设，选择颜色记住上次挑的那套，
 *  跟随歌曲按封面色挑，自定义图片铺图。背景轴（纯色/图/封面）由状态层一并推导。 */
function pickSource(source:ThemeSource):void {
 props.callbacks.themeSource(source);
 if (source === "image" && !props.state.background) {
  // 切过去但还没有图：直接把选图框弹出来，不然只是一片空底色
  props.callbacks.chooseBackground();
  return;
 }
 const following = source === "follow";
 if (followSong.value !== following) {
  followSong.value = following;
  writeString(FOLLOW_SONG_KEY, following ? "1" : "0");
  emitFollowSongChange(following);
 }
 if (source === "dark") paletteId.value = DEFAULT_PALETTE;
 else if (source === "light") paletteId.value = "paper";
 else if (source === "picked") paletteId.value = readString(PALETTE_KEY) || DEFAULT_PALETTE;
 if (source !== "image") emitPaletteChange(paletteId.value);
 // 「选择颜色」点下去就直接把颜色弹窗打开：设置页外观区不再有第二个「颜色」入口
 if (source === "picked") openColor();
}
// 下面三个是旧「自定义图 / 跟随封面 / 跟随歌曲」勾选框的兼容入口（节点保留给 G0 断言），
// 现在统一走 pickSource：勾上 = 切到对应主题，取消 = 退回黑白。
function toggleCustomImage():void {
 pickSource(themeSource.value === "image" ? fallbackSource() : "image");
}
function toggleCover():void {
 pickSource(themeSource.value === "follow" ? fallbackSource() : "follow");
}
function toggleFollowSong():void {
 pickSource(followSong.value ? fallbackSource() : "follow");
}
</script>
<template>
      <div class="settings-panel panel" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <FloatingBackdrop v-if="(surfaceVisible ?? state.session.panelOpen) && background && state.backgroundMode !== 'solid'" :model="background" :on-geometry="onGeometry" />
        <header class="settings-header">
          <div>
            <h2 id="settings-title">设置</h2>
            <p>主题、背景与音频设置</p>
          </div>
          <button id="close-settings-button" class="tool-button" type="button" aria-label="关闭设置">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6.4 5 5.6 5.6L17.6 5 19 6.4 13.4 12l5.6 5.6-1.4 1.4-5.6-5.6L6.4 19 5 17.6l5.6-5.6L5 6.4Z" /></svg>
          </button>
        </header>

        <div class="settings-content">
        <p class="settings-group-title">外观</p>
        <div class="appearance-block">
          <!-- 主题五选一：选哪个，主窗 / 小组件 / 任务栏三处一起换；另外四项只是变暗，照样能点 -->
          <div class="setting-row setting-row-stack">
            <span class="setting-row-label">主题</span>
            <div class="theme-row theme-row-five" role="radiogroup" aria-label="主题">
              <button class="theme-option theme-option-wide" type="button" data-theme-option="dark" :class="{ 'is-selected': field('themeSource:dark').selected }" :aria-checked="field('themeSource:dark').selected === undefined ? undefined : field('themeSource:dark').selected" @click="pickSource('dark')">
                高级模式
              </button>
              <button class="theme-option theme-option-wide" type="button" data-theme-option="light" :class="{ 'is-selected': field('themeSource:light').selected }" :aria-checked="field('themeSource:light').selected === undefined ? undefined : field('themeSource:light').selected" @click="pickSource('light')">
                超级高级模式
              </button>
              <button class="theme-option theme-option-wide" type="button" data-theme-option="picked" :class="{ 'is-selected': field('themeSource:picked').selected }" :aria-checked="field('themeSource:picked').selected === undefined ? undefined : field('themeSource:picked').selected" @click="pickSource('picked')">
                选择颜色
              </button>
              <button class="theme-option theme-option-wide" type="button" data-theme-option="follow" :class="{ 'is-selected': field('themeSource:follow').selected }" :aria-checked="field('themeSource:follow').selected === undefined ? undefined : field('themeSource:follow').selected" @click="pickSource('follow')">
                跟随歌曲
              </button>
              <button class="theme-option theme-option-wide" type="button" data-theme-option="image" :class="{ 'is-selected': field('themeSource:image').selected }" :aria-checked="field('themeSource:image').selected === undefined ? undefined : field('themeSource:image').selected" @click="pickSource('image')">
                自定义图片
              </button>
            </div>
          </div>
          <div id="image-settings" class="setting-row setting-row-stack">
            <span class="setting-row-label">图片</span>
            <div class="background-options">
              <button id="choose-background-button" @click="callbacks.chooseBackground()" class="small-button quiet" type="button" :disabled="field('chooseBackgroundButton').disabled ?? false">选择图片…</button>
              <button id="reset-background-button" @click="callbacks.resetBackground()" class="small-button quiet" type="button">恢复默认</button>
              <small id="background-name" class="background-name" :title="field('backgroundName').title ?? undefined">{{ field('backgroundName').text ?? "还没有选图" }}</small>
              <!-- 旧勾选框保留为 G0 断言节点，视觉并入上面「主题」那一行，不再单独展示 -->
              <label class="checkbox-row background-check compat-hidden" aria-hidden="true">
                <input id="background-custom-check" type="checkbox" tabindex="-1" :checked="state.themeSource === 'image'" @change="toggleCustomImage()" />
                自定义图
              </label>
              <label class="checkbox-row background-check compat-hidden" aria-hidden="true">
                <input id="background-cover-check" type="checkbox" tabindex="-1" :checked="state.themeSource === 'follow'" @change="toggleCover()" />
                跟随封面
              </label>
            </div>
          </div>
          <label id="texture-settings" class="slider-row" :style="{'--settings-fill': `${Number(field('glassBlurSlider').value ?? 40) / .8}%`}">
            <span>毛玻璃模糊 <output id="glass-blur-value">{{ field('glassBlurValue').text ?? "40px" }}</output></span>
            <input id="glass-blur-slider" @input="callbacks.slider('blur', ($event.target as HTMLInputElement).value)" @change="callbacks.sliderCommit('blur', ($event.target as HTMLInputElement).value)" v-field="field('glassBlurSlider')" type="range" min="0" max="80" value="40" />
          </label>
          <!-- 兼容节点保留原监听与G0；批准重建基线后可删除，不再读取或应用旧不透明度。 -->
          <label class="slider-row panel-alpha-compat" style="display:none" aria-hidden="true">
            <span>面板不透明度 <output id="panel-alpha-value">{{ field('panelAlphaValue').text ?? "72%" }}</output></span>
            <input id="panel-alpha-slider" @input="callbacks.slider('alpha', ($event.target as HTMLInputElement).value)" v-field="field('panelAlphaSlider')" type="range" min="20" max="100" value="72" tabindex="-1" aria-hidden="true" />
          </label>
          <label class="slider-row" :style="{'--settings-fill': `${Number(field('backgroundDimSlider').value ?? DEFAULT_BACKGROUND_DIM) / .95}%`}">
            <span>背景遮罩 <output id="background-dim-value">{{ field('backgroundDimValue').text ?? `${DEFAULT_BACKGROUND_DIM}%` }}</output></span>
            <input id="background-dim-slider" @input="callbacks.slider('dim', ($event.target as HTMLInputElement).value)" @change="callbacks.sliderCommit('dim', ($event.target as HTMLInputElement).value)" v-field="field('backgroundDimSlider')" type="range" min="0" max="95" value="30" />
          </label>
        </div>

        <p class="settings-group-title">功能</p>

        <details class="setting-group collapsible-group">
          <summary><h3>音频</h3></summary>
          <div class="setting-subgroup audio-source-section">
            <label class="select-row audio-source-row" for="stream-source-select">
              <span>音频获取方式</span>
              <select id="stream-source-select" @change="callbacks.source(($event.target as HTMLInputElement).value)" v-field="field('streamSourceSelect')" class="settings-select" :disabled="field('streamSourceSelect').disabled ?? false">
                <option value="guest" title="游客直连（免登录，推荐）">游客直连</option>
                <option value="auto" :disabled="field('autoSource').disabled" :title="field('autoSource').title ?? '自动（游客优先，失败用 yt-dlp 兜底）'">自动兜底</option>
              </select>
            </label>
            <small id="stream-source-status">{{ field('streamSourceStatus').text ?? "" }}</small>
            <small class="setting-note">决定从 B 站获取音频的方式。游客直连免登录、开箱即用；自动模式在获取失败时用 yt-dlp 兜底，需自备 yt-dlp.exe。</small>
          </div>
          <div class="setting-subgroup">
            <label class="checkbox-row" for="loudness-normalization-toggle">
              <input id="loudness-normalization-toggle" v-field="{checked:normalization,checkedVersion:normalizationVersion}" @change="callbacks.normalization(($event.target as HTMLInputElement).checked)" type="checkbox" aria-describedby="loudness-normalization-note" />
              响度归一化
            </label>
            <small id="loudness-normalization-note" class="setting-note">降低已测量歌曲之间的音量差异，不放大较轻的歌曲。未测量的歌曲保持原音量。</small>
          </div>
          <div class="setting-subgroup audio-cache-section">
            <label class="checkbox-row" for="audio-cache-enabled">
              <input id="audio-cache-enabled" @change="callbacks.cache()" v-field="field('audioCacheEnabled')" type="checkbox" aria-describedby="audio-cache-note" :disabled="field('audioCacheEnabled').disabled ?? true" />
              启用本地音频缓存
            </label>
            <small id="audio-cache-note" class="setting-note">缓存播放过的音频，可提升再次播放的速度和稳定性。缓存会占用磁盘空间，可随时清除。</small>
            <div class="audio-cache-details">
              <div class="audio-cache-row audio-cache-usage-row">
                <span>缓存占用</span>
                <small id="audio-cache-usage" class="setting-note audio-cache-usage">{{ field('audioCacheUsage').text ?? "读取中…" }}</small>
                <button id="clear-audio-cache-button" @click="callbacks.clearCache()" class="audio-cache-clear" type="button" :disabled="field('clearAudioCacheButton').disabled ?? false">清空缓存</button>
              </div>
              <label class="audio-cache-row" for="audio-cache-capacity">
                <span>缓存上限</span>
                <select id="audio-cache-capacity" @change="callbacks.cache()" v-field="field('audioCacheCapacity')" class="settings-select" :disabled="field('audioCacheCapacity').disabled ?? true">
                  <option value="536870912">512 MB</option>
                  <option value="1073741824">1 GB</option>
                  <option value="2147483648" selected>2 GB</option>
                  <option value="5368709120">5 GB</option>
                  <option value="10737418240">10 GB</option>
                </select>
              </label>
            </div>
            <small id="audio-cache-status" class="setting-note audio-cache-status" role="status" aria-live="polite">{{ field('audioCacheStatus').text ?? "" }}</small>
          </div>
          <div class="setting-subgroup shuffle-collection-section">
            <label class="shuffle-collection-row" for="shuffle-collection-order">
              <span>随机时合集内顺序</span>
              <select id="shuffle-collection-order" @change="callbacks.collection('order', ($event.target as HTMLInputElement).value)" v-field="field('shuffleCollectionOrder')" class="settings-select">
                <option value="random">随机</option>
                <option value="sequential">按原顺序</option>
              </select>
            </label>
            <label class="shuffle-collection-row" for="shuffle-collection-limit">
              <span>随机时每个合集播放</span>
              <select id="shuffle-collection-limit" @change="callbacks.collection('limit', ($event.target as HTMLInputElement).value)" v-field="field('shuffleCollectionLimit')" class="settings-select">
                <option value="all">全部</option>
                <option value="1">1 首</option>
                <option value="3">3 首</option>
                <option value="5">5 首</option>
                <option value="10">10 首</option>
              </select>
            </label>
            <small class="setting-note">开启随机播放时，遇到多P视频（合集）的播放方式。可随时点分P标识旁的跳过按钮跳过当前合集。</small>
          </div>
        </details>

        <details class="setting-group ai-settings collapsible-group" :ref="aiDetailsRef">
          <summary><h3>AI 推荐</h3></summary>
          <label class="settings-field" for="ai-api-format-select">
            <span>接口规范</span>
            <select id="ai-api-format-select" @change="callbacks.aiFormat()" v-field="field('aiApiFormatSelect')" class="settings-select">
              <option value="openai-chat-completions">OpenAI Chat Completions</option>
              <option value="openai-responses">OpenAI Responses</option>
              <option value="anthropic-messages">Anthropic Messages</option>
            </select>
          </label>
          <label class="settings-field" for="ai-model-input">
            <span>模型</span>
            <input id="ai-model-input" v-field="field('aiModelInput')" class="settings-input" type="text" autocomplete="off" placeholder="gpt-4o-mini" />
          </label>
          <label class="settings-field" for="ai-api-key-input">
            <span>API Key</span>
            <input id="ai-api-key-input" :ref="draftRef" class="settings-input" type="password" autocomplete="new-password" placeholder="留空则保留已保存的 key" />
          </label>
          <label class="settings-field" for="ai-base-url-input">
            <span>Base URL<span
                  class="ai-help-trigger"
                  tabindex="0"
                  role="button"
                  aria-label="Base URL 填写说明"
                  aria-describedby="ai-help-bubble">?<span
                    id="ai-help-bubble"
                    class="ai-help-bubble"
                    role="tooltip" :style="background?.surfaces?.help"><FloatingBackdrop v-if="(surfaceVisible ?? state.session.panelOpen) && background && state.backgroundMode !== 'solid'" :model="background" :on-geometry="onGeometry" /><strong>规则</strong>
                    <ul>
                      <li>请填到服务商要求的版本段，例如 https://api.openai.com/v1</li>
                      <li>只追加所选规范的端点，不自动补 /v1</li>
                      <li>缺少协议头时补 https://，末尾的 / 会去掉</li>
                    </ul>
                    <strong>示例</strong>
                    <ul>
                      <li>OpenAI：https://api.openai.com/v1</li>
                      <li>智谱：open.bigmodel.cn/api/v1</li>
                      <li>智谱 Anthropic：open.bigmodel.cn/api/anthropic/v1</li>
                      <li>百炼：dashscope.aliyuncs.com/compatible-mode/v1</li>
                    </ul></span></span></span>
            <input id="ai-base-url-input" :placeholder="field('aiBaseUrlInput').placeholder ?? 'https://api.openai.com/v1'" v-field="field('aiBaseUrlInput')" class="settings-input" type="url" autocomplete="off" />
          </label>
          <small class="setting-note">鼠标悬浮 Base URL 旁的“?”可查看填写说明</small>
          <div class="setting-actions">
            <button id="save-ai-config-button" @click="callbacks.saveAi()" class="secondary-button" type="button" :disabled="field('saveAiConfigButton').disabled ?? false">保存 AI 配置</button>
            <button id="test-ai-connection-button" @click="callbacks.testAi()" class="small-button quiet" type="button" :disabled="field('testAiConnectionButton').disabled ?? false">测试连接</button>
          </div>
          <small id="ai-config-status">{{ field('aiConfigStatus').text ?? "尚未读取 AI 配置" }}</small>
        </details>

        <details class="setting-group collapsible-group">
          <summary><h3>快捷键</h3></summary>
          <div class="setting-subgroup shortcut-row">
            <h4>上一曲</h4>
            <div class="shortcut-binding-actions">
              <button class="secondary-button shortcut-record-button" type="button" data-shortcut-action="previous" :class="{ 'is-recording':field('shortcut:previous').selected }" @click="callbacks.shortcut('previous')">{{ field('shortcut:previous').text ?? '未设置' }}</button>
              <button class="small-button quiet" type="button" data-shortcut-clear="previous" @click="callbacks.clearShortcut('previous')">清除</button>
            </div>
          </div>
          <div class="setting-subgroup shortcut-row">
            <h4>播放/暂停</h4>
            <div class="shortcut-binding-actions">
              <button class="secondary-button shortcut-record-button" type="button" data-shortcut-action="playPause" :class="{ 'is-recording':field('shortcut:playPause').selected }" @click="callbacks.shortcut('playPause')">{{ field('shortcut:playPause').text ?? '未设置' }}</button>
              <button class="small-button quiet" type="button" data-shortcut-clear="playPause" @click="callbacks.clearShortcut('playPause')">清除</button>
            </div>
          </div>
          <div class="setting-subgroup shortcut-row">
            <h4>下一曲</h4>
            <div class="shortcut-binding-actions">
              <button class="secondary-button shortcut-record-button" type="button" data-shortcut-action="next" :class="{ 'is-recording':field('shortcut:next').selected }" @click="callbacks.shortcut('next')">{{ field('shortcut:next').text ?? '未设置' }}</button>
              <button class="small-button quiet" type="button" data-shortcut-clear="next" @click="callbacks.clearShortcut('next')">清除</button>
            </div>
          </div>
          <div class="setting-subgroup shortcut-row">
            <h4>音量增加</h4>
            <div class="shortcut-binding-actions">
              <button class="secondary-button shortcut-record-button" type="button" data-shortcut-action="volumeUp" :class="{ 'is-recording':field('shortcut:volumeUp').selected }" @click="callbacks.shortcut('volumeUp')">{{ field('shortcut:volumeUp').text ?? '未设置' }}</button>
              <button class="small-button quiet" type="button" data-shortcut-clear="volumeUp" @click="callbacks.clearShortcut('volumeUp')">清除</button>
            </div>
          </div>
          <div class="setting-subgroup shortcut-row">
            <h4>音量降低</h4>
            <div class="shortcut-binding-actions">
              <button class="secondary-button shortcut-record-button" type="button" data-shortcut-action="volumeDown" :class="{ 'is-recording':field('shortcut:volumeDown').selected }" @click="callbacks.shortcut('volumeDown')">{{ field('shortcut:volumeDown').text ?? '未设置' }}</button>
              <button class="small-button quiet" type="button" data-shortcut-clear="volumeDown" @click="callbacks.clearShortcut('volumeDown')">清除</button>
            </div>
          </div>
          <small class="setting-note">系统级快捷键在软件处于后台时也会生效，支持 Windows 与 macOS。</small>
        </details>

        <details class="setting-group collapsible-group">
          <summary><h3>数据与隐私</h3></summary>
          <div class="setting-subgroup">
            <h4>数据备份</h4>
            <div class="setting-actions">
              <button id="export-data-button" :disabled="state.controls.dataBusy.backupExport" @click="callbacks.exportData()" class="secondary-button" type="button">导出数据</button>
              <button id="import-data-button" :disabled="state.controls.dataBusy.backupImport" @click="callbacks.importData()" class="secondary-button" type="button">导入数据</button>
            </div>
          </div>
          <div class="setting-subgroup">
            <h4>搜索历史</h4>
            <div class="setting-actions">
              <button id="clear-search-history-button" :disabled="state.controls.dataBusy.history" @click="callbacks.clearHistory()" class="small-button quiet" type="button">清空搜索历史</button>
            </div>
          </div>
          <div class="setting-subgroup">
            <h4>失效曲目</h4>
            <div class="setting-actions">
              <button id="purge-unavailable-tracks-button" :disabled="purgeBusy" @click="callbacks.purge()" class="small-button quiet" type="button">清理失效曲目</button>
            </div>
          </div>
          <small class="setting-note">导出的备份包含收藏、歌单、听歌记录、AI 配置与背景图，但不包含 AI API Key，在新设备导入后需要重新填写；导入会覆盖当前数据。清空搜索历史仅删除本地搜索记录，不影响收藏、歌单或播放队列。清理失效曲目会从收藏和所有歌单中移除这些歌曲。</small>
        </details>

        <p id="appearance-status" class="appearance-status" role="status">{{ appearanceStatus }}</p>
        </div>

        <div v-if="colorOpen" class="palette-dialog-mask" @click.self="colorOpen=false">
          <div class="palette-dialog palette-dialog-wide" role="dialog" aria-modal="true" aria-label="自定义颜色">
            <h3>自定义颜色</h3>
            <div class="palette-tabs" role="tablist" aria-label="颜色来源">
              <button class="palette-tab" type="button" role="tab" :class="{ 'is-active': colorTab === 'pick' }" :aria-selected="colorTab === 'pick'" @click="colorTab = 'pick'">选择颜色</button>
              <button class="palette-tab" type="button" role="tab" :class="{ 'is-active': colorTab === 'custom' }" :aria-selected="colorTab === 'custom'" @click="colorTab = 'custom'">自定义颜色</button>
            </div>
            <template v-if="colorTab === 'pick'">
              <label class="follow-row auto-palette-row">
                <input id="auto-palette-check" type="checkbox" class="follow-check" :checked="followSong" @change="toggleFollowSong()" />
                <span class="follow-switch" aria-hidden="true"></span>
                <span class="follow-text">自动配色<em>跟着当前歌曲封面挑一套</em></span>
              </label>
              <strong class="mini-theme-label">单色 · 12</strong>
              <div class="theme-row" :class="{ 'is-muted': followSong }" role="radiogroup" aria-label="单色配色">
                <label v-for="p in PALETTES.filter(x => x.group === 'solid')" :key="p.id" class="accent-option">
                  <input class="sr-only" type="radio" name="palette" :value="p.id" :checked="paletteId === p.id" @change="setPalette(p.id)" />
                  <span class="accent-dot mini-skin-dot" aria-hidden="true" :style="{background: previewOf(p.id)}"></span>
                  <span class="accent-option-name">{{ p.label }}</span>
                </label>
              </div>
              <strong class="mini-theme-label">渐变 · 28</strong>
              <div class="theme-row" :class="{ 'is-muted': followSong }" role="radiogroup" aria-label="渐变配色">
                <label v-for="p in PALETTES.filter(x => x.group === 'gradient')" :key="p.id" class="accent-option">
                  <input class="sr-only" type="radio" name="palette-gradient" :value="p.id" :checked="paletteId === p.id" @change="setPalette(p.id)" />
                  <span class="accent-dot mini-skin-dot" aria-hidden="true" :style="{background: previewOf(p.id)}"></span>
                  <span class="accent-option-name">{{ p.label }}</span>
                </label>
              </div>
              <p class="setting-hint">勾上自动配色时，下面 40 套灰掉；取消就回到手选。</p>
              <div class="setting-actions palette-dialog-actions">
                <span class="palette-dialog-gap"></span>
                <button class="small-button quiet" type="button" @click="colorOpen=false">取消</button>
                <button class="secondary-button" type="button" @click="colorOpen=false">完成</button>
              </div>
            </template>
            <template v-else>
              <p class="setting-hint">拖两个滑杆调颜色，下面那一条就是任务栏条的实际样子。也可以直接让程序从一张图里取。</p>
              <label class="slider-row" :style="{'--settings-fill': `${Math.round(customHue / 360 * 100)}%`}">
                <span>色相 <output>{{ customHue }}°</output></span>
                <input type="range" min="0" max="360" :value="customHue" @input="customHue = Number(($event.target as HTMLInputElement).value)" />
              </label>
              <label class="slider-row" :style="{'--settings-fill': `${Math.round((customSat - 5) / 85 * 100)}%`}">
                <span>饱和度 <output>{{ customSat }}%</output></span>
                <input type="range" min="5" max="90" :value="customSat" @input="customSat = Number(($event.target as HTMLInputElement).value)" />
              </label>
              <div class="palette-preview" :style="{ background: customSwatch.bar }">
                <span class="palette-preview-disc" :style="{ background: customSwatch.accent }"></span>
                <span class="palette-preview-text" :style="{ color: customSwatch.text }">夏日远去之后《明日方舟》</span>
                <span class="palette-preview-play" :style="{ background: customSwatch.accent }"></span>
              </div>
              <div class="setting-actions palette-dialog-actions">
                <button class="secondary-button" type="button" @click="customFromImage()">从图片取色</button>
                <span class="palette-dialog-gap"></span>
                <button class="small-button quiet" type="button" @click="colorOpen=false">取消</button>
                <button class="secondary-button" type="button" @click="applyCustom()">应用</button>
              </div>
            </template>
          </div>
        </div>
      </div>
</template>
