// Display preferences applied to <html>; styles.css reads the data attributes.
import type { AppSettings } from "./lib/ipc";

const TV_KEY = "mon.tvMode";
// ponytail: "Modo TV" is a per-screen display preference kept in this WebView's storage; move it to
// AppSettings if the coordinator adds the field (proposed), so it travels with the database.
export function getTvMode(): boolean {
  try {
    return localStorage.getItem(TV_KEY) === "1";
  } catch {
    return false;
  }
}
export function setTvMode(on: boolean) {
  try {
    localStorage.setItem(TV_KEY, on ? "1" : "0");
  } catch {
    /* storage unavailable: applies for this session only */
  }
  applyTvMode(on);
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

const CONSENT_KEY = "mon.updateConsentAsked";
// ponytail: same as above — proposed as an AppSettings field; until then the WebView remembers it.
export function updateConsentAsked(): boolean {
  try {
    return localStorage.getItem(CONSENT_KEY) === "1";
  } catch {
    return false;
  }
}
export function markUpdateConsentAsked() {
  try {
    localStorage.setItem(CONSENT_KEY, "1");
  } catch {
    /* ignore */
  }
}
