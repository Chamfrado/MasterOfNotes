// Display preferences applied to <html>; styles.css reads the data attributes.
// Source of truth: AppSettings.tvMode / updateConsentAsked (Law 04).
import { updateSettings, type AppSettings } from "./lib/ipc";
import { errorMessage } from "./components/ui";

// ponytail: TEMPORARY fallback until Rust persists tvMode/updateConsentAsked (coordinator will notify):
// getSettings may omit them (undefined → localStorage value, default false) and update_settings may refuse them.
// Remove the LEGACY_* helpers and the localStorage writes once the fields are live.
const LEGACY_TV = "mon.tvMode";
const LEGACY_CONSENT = "mon.updateConsentAsked";
function legacyGet(key: string): boolean {
  try {
    return localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}
function legacySet(key: string, on: boolean) {
  try {
    localStorage.setItem(key, on ? "1" : "0");
  } catch {
    /* storage unavailable */
  }
}

type Prefs = Partial<Pick<AppSettings, "tvMode" | "updateConsentAsked">>;
export const tvModeOf = (s: Prefs) => s.tvMode ?? legacyGet(LEGACY_TV);
export const consentAskedOf = (s: Prefs) => s.updateConsentAsked ?? legacyGet(LEGACY_CONSENT);

/** Persist a display pref in AppSettings; while Rust lacks the field, the legacy copy keeps it. */
async function persist(patch: Prefs) {
  try {
    await updateSettings(patch);
  } catch (e) {
    console.warn("update_settings (display pref, legacy fallback in use):", errorMessage(e));
  }
}

export function setTvMode(on: boolean) {
  applyTvMode(on);
  legacySet(LEGACY_TV, on);
  return persist({ tvMode: on });
}
export function markUpdateConsentAsked() {
  legacySet(LEGACY_CONSENT, true);
  return persist({ updateConsentAsked: true });
}

export function applyTvMode(on: boolean) {
  if (on) document.documentElement.dataset.tv = "on";
  else delete document.documentElement.dataset.tv;
}

/** "system" removes the override so prefers-reduced-motion decides. */
export function applyReducedMotion(mode: AppSettings["reducedMotion"]) {
  if (mode === "system") delete document.documentElement.dataset.reducedMotion;
  else document.documentElement.dataset.reducedMotion = mode;
}
