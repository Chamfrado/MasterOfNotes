// T-010-05: accessibility and 10-foot audit of the existing screens (SPEC-010), with the test-only IPC mock.
// Measurements are logged (`--reporter=list` shows them); findings go to docs/testing/evidence/a11y-audit.md.
import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { fullSpec, settings } from "./fixtures";
import { mockTauri, useSyntheticVideo, type MockSpec } from "./tauri-mock";

const nav = (page: Page, label: string) =>
  page.getByRole("navigation", { name: "Principal" }).getByRole("button", { name: label }).click();

const openPlayer = async (page: Page) => {
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Cantar Canção da Manhã Banda Aurora" }).click();
  await expect(page.getByRole("button", { name: "Começar" })).toBeVisible();
};

interface ScreenDef {
  name: string;
  /** Length of the synthetic video, when the screen needs one that plays. */
  videoMs?: number;
  reach: (page: Page) => Promise<void>;
  /** Where Esc must lead. */
  backTo: ((page: Page) => Promise<void>) | null;
}

const homeVisible = (page: Page) => expect(page.getByRole("heading", { name: "Preparação" })).toBeVisible();
const libraryVisible = (page: Page) => expect(page.getByRole("heading", { level: 1, name: "Biblioteca" })).toBeVisible();

const screens: ScreenDef[] = [
  { name: "Início", reach: homeVisible, backTo: null },
  {
    name: "Biblioteca",
    reach: async (p) => {
      await nav(p, "Biblioteca");
      await expect(p.getByText("3 músicas", { exact: true })).toBeVisible();
    },
    backTo: homeVisible,
  },
  {
    name: "Microfone",
    reach: async (p) => {
      await nav(p, "Microfone");
      await expect(p.getByLabel("Dispositivo de entrada")).toBeVisible();
    },
    backTo: homeVisible,
  },
  { name: "Player", videoMs: 800, reach: openPlayer, backTo: libraryVisible },
  {
    name: "Player pausado",
    videoMs: 6000,
    reach: async (p) => {
      await openPlayer(p);
      await p.getByRole("button", { name: "Começar" }).click();
      await p.getByRole("button", { name: "Pausar" }).click({ timeout: 10_000 });
      await expect(p.getByRole("button", { name: "Continuar" })).toBeVisible();
    },
    backTo: libraryVisible,
  },
  {
    name: "Resultado",
    videoMs: 800,
    reach: async (p) => {
      await openPlayer(p);
      await p.getByRole("button", { name: "Começar" }).click();
      await expect(p.getByRole("heading", { level: 1, name: "Resultado" })).toBeVisible({ timeout: 15_000 });
    },
    backTo: async (p) => expect(p.getByRole("heading", { level: 1, name: "Resultado" })).toHaveCount(0),
  },
  {
    name: "Ranking",
    reach: async (p) => {
      await nav(p, "Ranking");
      await expect(p.getByRole("table")).toBeVisible();
    },
    backTo: homeVisible,
  },
];

async function open(page: Page, s: ScreenDef, spec: MockSpec = {}) {
  const tracker = await mockTauri(page, fullSpec(spec));
  await page.goto("/");
  if (s.videoMs) await useSyntheticVideo(page, s.videoMs);
  await s.reach(page);
  return tracker;
}

// ---------- (a) axe-core ----------

const AXE_TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

for (const s of screens) {
  test(`axe: ${s.name} — zero violações serious/critical`, async ({ page }) => {
    await open(page, s);
    const { violations } = await new AxeBuilder({ page }).withTags(AXE_TAGS).analyze();
    for (const v of violations) {
      console.log(`[axe] ${s.name}: ${v.impact} ${v.id} (${v.nodes.length}×) ${v.help} — ${v.nodes[0]?.target.join(" ")}`);
    }
    const blocking = violations.filter((v) => v.impact === "serious" || v.impact === "critical").map((v) => `${v.impact} ${v.id}`);
    expect(blocking).toEqual([]);
  });
}

// ---------- (b) keyboard ----------

/** Visible, enabled elements in the sequential Tab order. */
const countTabbable = (page: Page) =>
  page.evaluate(
    () =>
      [...document.querySelectorAll<HTMLElement>("a[href], button, input, select, textarea, [tabindex]")].filter(
        (el) =>
          el.tabIndex >= 0 &&
          !(el as HTMLButtonElement).disabled &&
          el.getClientRects().length > 0 &&
          getComputedStyle(el).visibility !== "hidden" &&
          !el.closest("[inert], dialog:not([open])"),
      ).length,
  );

const focusInfo = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body || el === document.documentElement) return null;
    const cs = getComputedStyle(el);
    const r = el.getBoundingClientRect();
    const label = el.getAttribute("aria-label") ?? el.textContent?.trim().slice(0, 40) ?? "";
    return {
      key: `${el.tagName.toLowerCase()}#${[...document.querySelectorAll("*")].indexOf(el)}`,
      label: `${el.tagName.toLowerCase()} "${label || (el as HTMLInputElement).name || el.id}"`,
      outline: cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) >= 2,
      inViewport: r.bottom > 0 && r.right > 0 && r.top < innerHeight && r.left < innerWidth,
    };
  });

for (const s of screens) {
  test(`teclado: ${s.name} — Tab percorre tudo com foco visível, sem armadilha; Esc volta`, async ({ page }) => {
    const { pageErrors } = await open(page, s);
    const n = await countTabbable(page);
    expect(n).toBeGreaterThan(0);
    const seen = new Set<string>();
    const problems: string[] = [];
    for (let i = 0; i < n + 1; i++) {
      await page.keyboard.press("Tab");
      const f = await focusInfo(page);
      if (!f) continue; // document between the last and first element
      seen.add(f.key);
      if (!f.outline) problems.push(`sem foco visível: ${f.label}`);
      if (!f.inViewport) problems.push(`foco fora da tela: ${f.label}`);
    }
    console.log(`[teclado] ${s.name}: ${n} alvos de Tab, ${seen.size} alcançados`);
    expect(problems).toEqual([]);
    expect(seen.size, "Tab não alcança todos os elementos (armadilha ou elemento inalcançável)").toBe(n);
    if (s.backTo) {
      await page.keyboard.press("Escape");
      await s.backTo(page);
    }
    expect(pageErrors).toEqual([]);
  });
}

/** Presses Tab until the focused element has `name` (accessible label or text). */
async function tabTo(page: Page, name: string) {
  for (let i = 0; i < 40; i++) {
    await page.keyboard.press("Tab");
    const label = await page.evaluate(() => {
      const el = document.activeElement;
      return el?.getAttribute("aria-label") ?? el?.textContent?.trim() ?? "";
    });
    if (label === name) return;
  }
  throw new Error(`Tab nunca chegou em "${name}"`);
}

test("teclado: fluxo Início → Biblioteca → Player → Resultado → Ranking só com Tab/Enter", async ({ page }) => {
  const { pageErrors } = await mockTauri(page, fullSpec());
  await page.goto("/");
  await useSyntheticVideo(page);
  await homeVisible(page);
  await tabTo(page, "Escolher música");
  await page.keyboard.press("Enter");
  await expect(page.getByText("3 músicas", { exact: true })).toBeVisible();
  await tabTo(page, "Cantar Canção da Manhã Banda Aurora");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("button", { name: "Começar" })).toBeFocused(); // autoFocus
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 1, name: "Resultado" })).toBeVisible({ timeout: 15_000 });
  await tabTo(page, "Ver ranking");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 1, name: "Ranking" })).toBeVisible();
  expect(pageErrors).toEqual([]);
});

// ---------- (c) viewports / 10-foot ----------

const viewports = [
  { width: 960, height: 600 },
  { width: 1366, height: 768 },
  { width: 1920, height: 1080 },
  { width: 3840, height: 2160 },
];
const MIN_TARGET_PX = 48;
const MIN_FONT_PX = 14;

const measureLayout = (page: Page, minTarget: number) =>
  page.evaluate((minTarget) => {
    const visible = (el: Element) => {
      const r = el.getBoundingClientRect();
      return r.width > 1 && r.height > 1 && getComputedStyle(el).visibility !== "hidden";
    };
    const describe = (el: Element) =>
      `${el.tagName.toLowerCase()} "${(el.getAttribute("aria-label") ?? el.textContent ?? "").trim().slice(0, 30)}"`;
    const smallTargets = [
      ...document.querySelectorAll("a[href], button, input:not([type=hidden]), select, textarea, [role=button], [tabindex]:not([tabindex='-1'])"),
    ]
      .filter(visible)
      .map((el) => ({ el, r: el.getBoundingClientRect() }))
      .filter(({ r }) => r.width < minTarget || r.height < minTarget)
      .map(({ el, r }) => `${describe(el)} ${Math.round(r.width)}×${Math.round(r.height)}`);
    let minFont = Infinity;
    let minFontAt = "";
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let t = walker.nextNode(); t; t = walker.nextNode()) {
      const el = t.parentElement;
      if (!el || !t.textContent?.trim() || !visible(el)) continue;
      const size = parseFloat(getComputedStyle(el).fontSize);
      if (size < minFont) [minFont, minFontAt] = [size, describe(el)];
    }
    const root = document.documentElement;
    return {
      overflowX: root.scrollWidth - root.clientWidth,
      smallTargets,
      minFont,
      minFontAt,
      rootFont: parseFloat(getComputedStyle(root).fontSize),
    };
  }, minTarget);

for (const s of screens) {
  test(`viewports: ${s.name} — sem overflow horizontal, alvos ≥ ${MIN_TARGET_PX}px, texto ≥ ${MIN_FONT_PX}px`, async ({ page }, info) => {
    await open(page, s);
    const problems: string[] = [];
    for (const vp of viewports) {
      await page.setViewportSize(vp);
      await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
      const m = await measureLayout(page, MIN_TARGET_PX);
      const tag = `${vp.width}×${vp.height}`;
      console.log(
        `[viewport] ${s.name} ${tag}: raiz ${m.rootFont}px, menor texto ${m.minFont}px (${m.minFontAt}), overflow ${m.overflowX}px, alvos pequenos ${m.smallTargets.length}`,
      );
      if (m.overflowX > 0) problems.push(`${tag}: overflow horizontal de ${m.overflowX}px`);
      for (const t of m.smallTargets) problems.push(`${tag}: alvo < ${MIN_TARGET_PX}px: ${t}`);
      if (m.minFont < MIN_FONT_PX) problems.push(`${tag}: texto de ${m.minFont}px em ${m.minFontAt}`);
      await page.screenshot({ path: info.outputPath(`${s.name}-${tag}.png`) });
    }
    expect(problems).toEqual([]);
  });
}

// ---------- (d) reduced motion ----------

/** Animations/transitions longer than the 0.01ms reset used by styles.css (keeps animationend firing). */
const longMotion = (page: Page) =>
  page.evaluate(() => {
    const ms = (list: string) => Math.max(...list.split(",").map((v) => (v.trim().endsWith("ms") ? parseFloat(v) : parseFloat(v) * 1000)));
    const out: string[] = [];
    for (const el of document.querySelectorAll("*")) {
      for (const pseudo of [null, "::before", "::after"]) {
        const cs = getComputedStyle(el, pseudo);
        const what = `${el.tagName.toLowerCase()}${pseudo ?? ""}.${String(el.getAttribute("class") ?? "").slice(0, 40)}`;
        if (cs.animationName !== "none" && ms(cs.animationDuration) > 0.01) out.push(`animation ${cs.animationDuration} ${what}`);
        if (ms(cs.transitionDuration) > 0.01) out.push(`transition ${cs.transitionDuration} ${what}`);
      }
    }
    for (const a of document.getAnimations()) {
      const d = a.effect?.getTiming().duration;
      if (typeof d === "number" && d > 0.01) out.push(`getAnimations ${d}ms`);
    }
    return out;
  });

const motionModes = [
  { mode: "preferência do sistema", media: "reduce" as const, setting: "system" as const },
  { mode: "ajuste do app", media: "no-preference" as const, setting: "on" as const },
];

for (const { mode, media, setting } of motionModes) {
  for (const s of screens) {
    test(`movimento reduzido (${mode}): ${s.name} sem animação nem transição`, async ({ page }) => {
      await page.emulateMedia({ reducedMotion: media });
      await open(page, s, { get_settings: { ok: settings("C:/Karaoke", { reducedMotion: setting }) } });
      expect(await longMotion(page)).toEqual([]);
    });
  }
}

test("controle negativo: sem redução de movimento, o Início anima (a checagem acima não é vazia)", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await open(page, screens[0], { get_settings: { ok: settings("C:/Karaoke", { reducedMotion: "off" }) } });
  expect((await longMotion(page)).length).toBeGreaterThan(0);
});
