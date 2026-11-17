import {attachSettingsControlPorts} from "./settings-controls-fixture.ts";
import { PALETTE_KEY, SKIN_KEY, activateCustomPalette, activePaletteId, applyPaletteToRoot, deactivateCustomPalette, isLightPalette, paletteById, readCustomPalette } from "../../ui/shared/palettes.ts";
import { paletteFromImage } from "../../ui/shared/custom-palette.ts";
import { livePalette, setLivePalette, subscribeLivePalette } from "../../ui/shared/live-palette.ts";
import path from "node:path";
import vm from "node:vm";
import { readFileSync } from "./module-syntax.ts";
const settingsUi = path.join(import.meta.dirname, "../../ui");
const settingsVmSource = ["features/pages/page-selection.ts", "features/settings/settings-state.ts", "features/settings/settings-view.ts", "features/settings/settings-adapter.ts"].map(file => readFileSync(path.join(settingsUi, file), "utf8")).join("\n");
function attachSettingsVm(context: vm.Context): void {
  context.setModalStage ??= (_kind:string,fields:{hidden?:boolean;open?:boolean})=>{const modal=context.settingsModal;if(fields.hidden !== undefined)modal.hidden=fields.hidden;if(fields.open !== undefined){modal.classList.toggle("is-open",fields.open);modal.setAttribute("aria-hidden",String(!fields.open));}};
  const node = () => ({ value: "", textContent: "", hidden: false, style: {}, dataset: {}, classList: { add() {}, remove() {}, toggle() {}, contains() { return false; } }, setAttribute() {}, removeAttribute() {}, contains() { return false; }, querySelector() { return null; } });
  for (const name of ["root","navItems","homeView","searchView","favoritesView","playlistsView","placeholderView","placeholderTitle","settingsModal","openSettingsButton","themeOptions","accentColorOptions","chooseBackgroundButton","backgroundName","appearanceStatus","imageOnlyGroups","glassBlurSlider","glassBlurValue","panelAlphaSlider","panelAlphaValue","backgroundDimSlider","backgroundDimValue","streamSourceSelect","streamSourceStatus","audioCacheEnabled","audioCacheCapacity","shuffleCollectionOrder","shuffleCollectionLimit","audioCacheUsage","audioCacheStatus","clearAudioCacheButton","aiApiFormatSelect","aiBaseUrlInput","aiModelInput","aiApiKeyInput","saveAiConfigButton","testAiConnectionButton","aiConfigStatus","shortcutRecordButtons"]) if (!(name in context)) context[name] = ["navItems", "themeOptions", "accentColorOptions", "imageOnlyGroups", "shortcutRecordButtons"].includes(name) ? [] : node();
  const markup=readFileSync(path.join(settingsUi,"features/settings/components/SettingsPanel.vue"),"utf8");
  for(const [name,id] of [["panelAlphaSlider","panel-alpha-slider"],["backgroundDimSlider","background-dim-slider"]]) {
    const min=new RegExp('id="'+id+'"[^>]*\\bmin="([^\"]+)"').exec(markup)?.[1];
    if(!min) throw Error("Missing real settings input min: "+id);
    context[name].min ??= min;
  }
  context.root.style.setProperty ??= () => {};
  context.root.style.removeProperty ??= () => {};
  // settings-view 通过 root-style.ts 往 <html> 写属性和变量；VM 拼接的源码剥掉了 import，
  // 所以这四个出口要按真实实现挂到 context 上，否则投影一执行就 ReferenceError。
  context.setRootData ??= (name: string, value: string) => { context.root.dataset[name] = value; };
  context.deleteRootData ??= (name: string) => { delete context.root.dataset[name]; };
  context.setRootVar ??= (name: string, value: string) => { context.root.style.setProperty(name, value); };
  context.removeRootVar ??= (name: string) => { context.root.style.removeProperty(name); };
  // surfaceTheme() 现在按「当前生效配色」的明暗出值（settings-state.ts 里调 livePalette），
  // 拼接源码剥掉 import 后必须把真实的 live 色源挂上去，否则投影一执行就 ReferenceError。
  context.livePalette ??= livePalette;
  context.setLivePalette ??= setLivePalette;
  context.subscribeLivePalette ??= subscribeLivePalette;
  // settings-state.ts 按「主题」五选一算配色与明暗轴，拼接源码剥掉 import 后这些出口都得挂上。
  context.paletteById ??= paletteById;
  context.isLightPalette ??= isLightPalette;
  context.applyPaletteToRoot ??= applyPaletteToRoot;
  context.activePaletteId ??= activePaletteId;
  context.readCustomPalette ??= readCustomPalette;
  context.activateCustomPalette ??= activateCustomPalette;
  context.deactivateCustomPalette ??= deactivateCustomPalette;
  context.paletteFromImage ??= paletteFromImage;
  context.PALETTE_KEY ??= PALETTE_KEY;
  context.SKIN_KEY ??= SKIN_KEY;
  context.document ??= { documentElement: context.root, activeElement: null };
  context.requestAnimationFrame ??= () => {};
  context.localStorage ??= { getItem: () => null, setItem() {}, removeItem() {} };
  context.getSettingsDomForTest = () => context;
  context.renderSettingsControlsForTest = attachSettingsControlPorts(context, true);
  vm.runInContext(settingsVmSource + `
let settingsForTest = initialSettingsState();
    if (typeof shortcutBindings !== "undefined") settingsForTest.shortcutBindings = { ...settingsForTest.shortcutBindings, ...shortcutBindings };
    const settingsActionsForTest = createSettingsActions(() => settingsForTest, next => { const previous = settingsForTest; settingsForTest = next; renderSettingsControlsForTest(next); renderSettingsSnapshot(next, previous, getSettingsDomForTest()); }, {
      invoke: (...args) => invokeAppearance(...args), report() {}, settings: {
        getItem: key => localStorage.getItem(key),
        setItem: (key, value) => localStorage.setItem(key === "bilibili-music.theme" && typeof THEME_KEY !== "undefined" ? THEME_KEY : key, value),
        removeItem: key => localStorage.removeItem(key),
        cancelCacheNotice: () => { if (typeof audioCacheStatusTimer !== "undefined") { if (audioCacheStatusTimer !== null) clearTimeout(audioCacheStatusTimer); audioCacheStatusTimer = null; } },
        scheduleCacheNotice: callback => setTimeout(callback, 3000),
      }
    });
    function getSettingsActions() { return settingsActionsForTest; }
    function getSettingsSnapshot() { return settingsForTest; }
    function settingsDom() { return getSettingsDomForTest(); }
  `, context);
}
export { attachSettingsVm };
