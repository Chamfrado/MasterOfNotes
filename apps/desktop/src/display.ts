// Display preferences applied to <html>; styles.css reads the data attributes.
// Source of truth: AppSettings.tvMode / updateConsentAsked, persisted by Rust (Law 04).

export function applyTvMode(on: boolean) {
  if (on) document.documentElement.dataset.tv = "on";
  else delete document.documentElement.dataset.tv;
}

/** "system" removes the override so prefers-reduced-motion decides. */
export function applyReducedMotion(mode: "system" | "on" | "off") {
  if (mode === "system") delete document.documentElement.dataset.reducedMotion;
  else document.documentElement.dataset.reducedMotion = mode;
}
