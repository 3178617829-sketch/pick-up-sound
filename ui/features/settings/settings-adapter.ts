import { normalizeShuffleCollectionPrefs } from "../pages/page-selection.ts";
import type { SettingsPlatform } from "./settings-state.ts";
import { SHUFFLE_LIMIT_KEY, SHUFFLE_ORDER_KEY, readString, removeKey, writeString } from "../../shared/storage.ts";
function readSettingsPreference(key: string): string | null {
  return readString(key);
}
function readSettingsCollectionPrefs(): ReturnType<typeof normalizeShuffleCollectionPrefs> {
  return normalizeShuffleCollectionPrefs(readString(SHUFFLE_ORDER_KEY), readString(SHUFFLE_LIMIT_KEY));
}
function createSettingsPlatform(): SettingsPlatform {
  let noticeTimer: ReturnType<typeof window.setTimeout> | null = null;
  return {
    getItem: key => readString(key), setItem: (key, value) => writeString(key, value), removeItem: key => removeKey(key), cancelCacheNotice() {
      if (noticeTimer !== null) clearTimeout(noticeTimer);
      noticeTimer = null;
    }, scheduleCacheNotice(callback) {
      noticeTimer = setTimeout(() => {
        callback();
        noticeTimer = null;
      }, 3000);
    }
  };
}
export { createSettingsPlatform, readSettingsCollectionPrefs, readSettingsPreference };
