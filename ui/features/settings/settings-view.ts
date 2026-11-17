import {setModalStage} from "../../app/shell-stages.ts";
import {removeRootVar, setRootData, setRootVar} from "../../shared/root-style.ts";
import {projectImageForeground,surfaceTheme} from "./settings-state.ts";
import type {SettingsSnapshot} from "./settings-state.ts";
import type {SettingsDom} from "./appearance.ts";
function prepareSettingsPanel(dom:SettingsDom):void { setModalStage("settings",{hidden:false}); }
function renderSettingsSnapshot(state:SettingsSnapshot,previous:SettingsSnapshot|null,dom:SettingsDom):void {
  const before=previous?.view,view=state.view;
  if(view.themeVersion && view.themeVersion !== before?.themeVersion) {
    const surface=surfaceTheme(state);
    setRootData("theme", surface);
    // 毛玻璃五种主题都要能调（用户 m01378-3）：这里只收掉动态质感的两项固定值，不碰模糊度
    if(surface === "dynamic") for(const property of ["--panel-alpha","--background-dim"]) removeRootVar(property);
  }
  if(view.sliders && view.sliders !== before?.sliders) {
    const kind=view.sliders.kind;
    if(kind === "all" || kind === "blur") setRootVar("--glass-blur",state.glassBlur+"px");
    if(kind === "all" || kind === "dim") setRootVar("--background-dim",String(state.backgroundDim/100));
  }
  // 强调色现在由配色层（applyPaletteToRoot）统一写 --accent-r/g/b；这里再写一次
  // 会用旧的 accentColor 覆盖回去，导致「播放键是配色的橙、导航高亮还是旧的绿」。
  // 图片底只在「自定义图片」主题下铺。切到黑/白/手选/跟随时即使还留着上次的图片数据，
  // 也必须把 --app-background 撤掉，否则换主题后背景图照样糊在下面。
  const backgroundChanged = (view.backgroundVersion && view.backgroundVersion !== before?.backgroundVersion) || (view.themeVersion && view.themeVersion !== before?.themeVersion);
  if(backgroundChanged) {
    if(state.background && state.backgroundMode === "image") setRootVar("--app-background",'url("'+state.background.dataUrl+'")');
    else removeRootVar("--app-background");
  }
  projectImageForeground(state);
  const session = state.session;
  if (session.panelVersion && session.panelVersion !== previous?.session.panelVersion) {
    if (session.panelOpen) requestAnimationFrame(() => {
      setModalStage("settings",{open:true});
    });
    else {
      if (dom.settingsModal.contains(document.activeElement)) dom.openSettingsButton.focus({
        preventScroll: true
      });
      setModalStage("settings",{open:false});
    }
  }
}
function finishSettingsPanelTransition(event: TransitionEvent, dom: SettingsDom): void {
  if (event.target === dom.settingsModal && !dom.settingsModal.classList.contains("is-open")) setModalStage("settings",{hidden:true});
}
export { finishSettingsPanelTransition, prepareSettingsPanel, renderSettingsSnapshot };
