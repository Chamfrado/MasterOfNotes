// T-010-05: accessibility and 10-foot audit of the existing screens (SPEC-010), with the test-only IPC mock.
// Measurements are logged (`--reporter=list` shows them); findings go to docs/testing/evidence/a11y-audit.md.
import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import type { AppSettings, GameSession, MelodyReference } from "../src/lib/ipc";
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
  /** The synthetic video carries an audio track (SPEC-012 decodes it). */
  videoAudio?: boolean;
  /** Known keyboard finding: the keyboard test is expected to fail until it is fixed (remove then). */
  knownKeyboardIssue?: string;
  /** Extra static replies for this screen. */
  spec?: MockSpec;
  reach: (page: Page) => Promise<void>;
  /** Where Esc must lead. */
  backTo: ((page: Page) => Promise<void>) | null;
}

const homeVisible = (page: Page) => expect(page.getByRole("heading", { name: "Preparação" })).toBeVisible();
const libraryVisible = (page: Page) => expect(page.getByRole("heading", { level: 1, name: "Biblioteca" })).toBeVisible();

// Data for the game and reference screens (static: this file audits screens, the flows live in party.e2e.ts).
const party: GameSession = {
  id: 10,
  mode: "party",
  name: "Festa de Sábado",
  status: "active",
  config: { minRounds: 1, difficulty: "normal", seed: 7 },
  startedMs: Date.UTC(2026, 10, 15, 21, 0),
  completedMs: null,
  rounds: [
    { id: 1, position: 1, songId: "c3", participantId: 1, status: "completed", score: 80 },
    { id: 2, position: 2, songId: "c3", participantId: 2, status: "pending", score: null },
    { id: 3, position: 3, songId: "c3", participantId: 1, status: "pending", score: null },
  ],
  standings: [
    { participantId: 1, total: 80, average: 80, rounds: 1, eligible: true, rank: 1 },
    { participantId: 2, total: 0, average: 0, rounds: 0, eligible: false, rank: 2 },
  ],
};
const duel: GameSession = {
  ...party,
  id: 11,
  mode: "duel",
  name: null,
  status: "completed",
  completedMs: Date.UTC(2026, 10, 14, 22, 0),
  startedMs: Date.UTC(2026, 10, 14, 21, 0),
  rounds: [
    { id: 4, position: 1, songId: "c3", participantId: 1, status: "completed", score: 77.5 },
    { id: 5, position: 2, songId: "c3", participantId: 2, status: "completed", score: 81 },
  ],
  standings: [],
};
const reference: MelodyReference = {
  id: 3,
  songId: "c3",
  source: "json",
  status: "validated",
  revision: 2,
  offsetMs: 0,
  notes: [
    { startMs: 2000, durMs: 600, midi: 60, lyric: "Mas" },
    { startMs: 2600, durMs: 600, midi: 62, lyric: "ter" },
  ],
};
const gameSpec: MockSpec = {
  list_participants: { ok: [{ id: 1, name: "Ana" }, { id: 2, name: "Bruno" }] },
  list_sessions: { ok: [party, duel] },
  get_session: { ok: party },
  get_reference: { ok: reference },
  list_midi_tracks: {
    ok: [
      { index: 0, name: "Piano", noteCount: 120 },
      { index: 1, name: "Voz", noteCount: 85 },
    ],
  },
  "plugin:dialog|open": { ok: "C:/Karaoke/c3.mid" },
};

const playVisible = (page: Page) => expect(page.getByRole("heading", { level: 1, name: "Jogar" })).toBeVisible();
const dialogClosed = (page: Page) => expect(page.locator("dialog[open]")).toHaveCount(0);
const openPlay = async (page: Page, button?: string | RegExp) => {
  await nav(page, "Jogar");
  await playVisible(page);
  if (button) await page.getByRole("button", { name: button }).click();
};
const openReference = async (page: Page) => {
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Referência de Pôr do Sol em Óculos" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Pôr do Sol em Óculos" })).toBeVisible();
};

/** T-012-05 finding (Agent 3): the alignment preview <video controls> gets focus with its ring clipped by the rounded overflow-hidden box. */
const F3 = "F3: o <video controls> da prévia recebe foco sem indicador visível (contorno cortado pelo overflow-hidden do contêiner)";
const withModel: MockSpec = {
  get_settings: { ok: settings("C:/Karaoke", { separationModel: { id: "5e1f", name: "Separador Exemplo v1", acceptedMs: Date.UTC(2026, 10, 20, 12) } }) },
  upload_song_audio: { ok: "tok" },
};
const openAutoReferenceSettings = async (page: Page) => {
  await nav(page, "Ajustes");
  await expect(page.getByRole("heading", { level: 2, name: "Referência automática" })).toBeVisible();
};

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
  {
    name: "Jogar",
    reach: async (p) => {
      await openPlay(p);
      await expect(p.getByRole("heading", { name: "Continuar de onde parou" })).toBeVisible();
    },
    backTo: homeVisible,
  },
  {
    name: "Participantes",
    reach: async (p) => {
      await openPlay(p, "Participantes");
      await expect(p.getByRole("list", { name: "Cantores" })).toBeVisible();
    },
    backTo: playVisible,
  },
  {
    name: "Nova partida",
    reach: async (p) => {
      await openPlay(p, /^Festa/);
      await expect(p.getByRole("heading", { level: 1, name: "Nova partida: Festa" })).toBeVisible();
    },
    backTo: playVisible,
  },
  {
    name: "Partida",
    reach: async (p) => {
      await openPlay(p, "Retomar");
      await expect(p.getByRole("heading", { name: "Ranking parcial" })).toBeVisible();
    },
    backTo: playVisible,
  },
  {
    name: "Diálogo Encerrar a partida",
    reach: async (p) => {
      await openPlay(p, "Retomar");
      await p.getByRole("button", { name: "Encerrar" }).click();
      await expect(p.getByRole("dialog", { name: "Encerrar a partida?" })).toBeVisible();
    },
    backTo: dialogClosed,
  },
  {
    name: "Histórico",
    reach: async (p) => {
      await openPlay(p, "Histórico");
      await expect(p.getByRole("button", { name: /Festa de Sábado/ })).toBeVisible();
    },
    backTo: playVisible,
  },
  { name: "Referência", reach: openReference, backTo: libraryVisible, knownKeyboardIssue: F3 },
  {
    name: "Diálogo Qual trilha é a voz",
    reach: async (p) => {
      await openReference(p);
      await p.getByRole("button", { name: "Importar outra" }).click();
      await expect(p.getByRole("dialog", { name: "Qual trilha é a voz?" })).toBeVisible();
    },
    backTo: dialogClosed,
  },
  {
    name: "Gravar cantando",
    reach: async (p) => {
      await openReference(p);
      await p.getByRole("button", { name: "Gravar cantando" }).click();
      await expect(p.getByText("A referência atual fica no histórico; a nova nasce experimental.")).toBeVisible();
    },
    backTo: libraryVisible,
  },
  {
    name: "Gravando notas",
    videoMs: 2000,
    reach: async (p) => {
      await openReference(p);
      await p.getByRole("button", { name: "Gravar cantando" }).click();
      await p.getByRole("button", { name: "Começar" }).click();
      await expect(p.getByText("Gravando notas")).toBeVisible({ timeout: 10_000 });
      await p.locator("video").evaluate((v: HTMLVideoElement) => v.pause()); // stays recording; the video never ends mid-audit
    },
    backTo: libraryVisible,
  },
  {
    name: "Diálogo Pular trecho",
    videoMs: 6000,
    reach: async (p) => {
      await openPlayer(p);
      await p.getByRole("button", { name: "Começar" }).click();
      await p.getByRole("button", { name: "Pausar" }).click({ timeout: 10_000 });
      await p.keyboard.press("ArrowRight"); // seek while competitive → asks first
      await expect(p.getByRole("dialog", { name: "Pular trecho?" })).toBeVisible();
    },
    backTo: dialogClosed,
  },
  // SPEC-012 (T-012-05)
  {
    name: "Ajustes: Referência automática sem modelo",
    reach: openAutoReferenceSettings,
    backTo: homeVisible,
  },
  {
    name: "Ajustes: Referência automática com modelo",
    spec: withModel,
    reach: async (p) => {
      await openAutoReferenceSettings(p);
      await expect(p.getByRole("button", { name: "Remover" })).toBeVisible();
    },
    backTo: homeVisible,
  },
  {
    name: "Gerar automaticamente em andamento",
    videoMs: 1000,
    videoAudio: true,
    spec: withModel,
    reach: async (p) => {
      await p.evaluate(() => void ((window.__E2E_HANDLERS__ ??= {}).generate_reference = () => new Promise(() => {}))); // never ends
      await openReference(p);
      await p.getByRole("button", { name: "Gerar automaticamente" }).click();
      await p.getByRole("button", { name: "Começar" }).click();
      await expect(p.getByRole("status").filter({ hasText: "Etapa 2 de 4: Separando a voz" })).toBeVisible();
    },
    backTo: libraryVisible,
  },
  {
    name: "Proposta de alinhamento",
    videoMs: 1000,
    videoAudio: true,
    spec: { upload_song_audio: { ok: "tok" }, auto_align_reference: { ok: { offsetMs: 1200, confidence: 0.3 } } },
    knownKeyboardIssue: F3,
    reach: async (p) => {
      await openReference(p);
      await p.getByRole("button", { name: "Alinhar automaticamente" }).click();
      await expect(p.getByText("Confiança baixa: confira pelo vídeo antes de aplicar.")).toBeVisible();
    },
    backTo: libraryVisible,
  },
];

async function open(page: Page, s: ScreenDef, spec: MockSpec = {}) {
  const tracker = await mockTauri(page, fullSpec({ ...gameSpec, ...s.spec, ...spec }));
  await page.goto("/");
  if (s.videoMs) await useSyntheticVideo(page, s.videoMs, s.videoAudio);
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
      // A modal dialog makes the rest of the page inert without marking it, so count only inside it.
      [...(document.querySelector("dialog:modal") ?? document).querySelectorAll<HTMLElement>("a[href], button, input, select, textarea, video[controls], audio[controls], [tabindex]")].filter(
        (el) =>
          el.tabIndex >= 0 &&
          !(el as HTMLButtonElement).disabled &&
          el.getClientRects().length > 0 &&
          getComputedStyle(el).visibility !== "hidden" &&
          !el.closest("[inert], dialog:not([open])") &&
          // A radio group is one Tab stop (the checked radio, else the first); arrows move inside it.
          !(
            el instanceof HTMLInputElement &&
            el.type === "radio" &&
            [...document.querySelectorAll<HTMLInputElement>(`input[type=radio][name="${el.name}"]`)].find((r) => r.checked || !r.form?.querySelector(`input[name="${el.name}"]:checked`)) !== el
          ),
      ).length,
  );

const focusInfo = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body || el === document.documentElement) return null;
    const cs = getComputedStyle(el);
    const r = el.getBoundingClientRect();
    const label = el.getAttribute("aria-label") ?? el.textContent?.trim().slice(0, 40) ?? "";
    const ring = cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) >= 2;
    // The ring around the element: is it cut by an ancestor that clips (overflow other than visible)?
    const grow = ring ? parseFloat(cs.outlineWidth) + parseFloat(cs.outlineOffset) : 0;
    let clipped = false;
    for (let a = el.parentElement; a && ring && grow > 0; a = a.parentElement) {
      const o = getComputedStyle(a);
      if (o.overflowX === "visible" && o.overflowY === "visible") continue;
      const b = a.getBoundingClientRect();
      clipped ||= r.left - grow < b.left || r.top - grow < b.top || r.right + grow > b.right || r.bottom + grow > b.bottom;
    }
    return {
      // Stable identity (a DOM index shifts when the page changes mid-cycle, e.g. a toast).
      key: (el.dataset.e2eFocus ??= String(document.querySelectorAll("[data-e2e-focus]").length)),
      label: `${el.tagName.toLowerCase()} "${label || (el as HTMLInputElement).name || el.id}"`,
      // Native media controls draw their own ring on the inner (closed shadow DOM) button: only the host is ours.
      outline: (ring && !clipped) || (el instanceof HTMLMediaElement && !el.matches(":focus-visible")),
      clipped,
      inViewport: r.bottom > 0 && r.right > 0 && r.top < innerHeight && r.left < innerWidth,
    };
  });

for (const s of screens) {
  test(`teclado: ${s.name} — Tab percorre tudo com foco visível, sem armadilha; Esc volta`, async ({ page }) => {
    test.fail(!!s.knownKeyboardIssue, s.knownKeyboardIssue);
    const { pageErrors } = await open(page, s);
    const n = await countTabbable(page);
    expect(n).toBeGreaterThan(0);
    const seen = new Set<string>();
    const problems = new Set<string>();
    // One full cycle. Native <video controls> add Tab stops inside its (closed) shadow DOM, all reported as
    // the <video>, so the loop stops when the first element comes back instead of after exactly n presses.
    let first: string | null = null;
    let prev: string | null = null;
    for (let i = 0; i < 3 * n + 10; i++) {
      await page.keyboard.press("Tab");
      const f = await focusInfo(page);
      if (!f) continue; // document between the last and first element
      if (f.key === first && prev !== first) break;
      first ??= f.key;
      prev = f.key;
      seen.add(f.key);
      if (!f.outline) problems.add(`sem foco visível${f.clipped ? " (contorno cortado por um ancestral com overflow)" : ""}: ${f.label}`);
      if (!f.inViewport) problems.add(`foco fora da tela: ${f.label}`);
    }
    console.log(`[teclado] ${s.name}: ${n} alvos de Tab, ${seen.size} alcançados`);
    expect([...problems]).toEqual([]);
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
      // A checkbox/radio wrapped in its <label> is clicked through the whole label (WCAG 2.5.8 "equivalent").
      .map((el) => ({ el, r: (el.matches("input[type=checkbox], input[type=radio]") ? (el.closest("label") ?? el) : el).getBoundingClientRect() }))
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
    expect([...problems]).toEqual([]);
  });
}

test("Modo TV (A11Y-06): checkbox e rádio desenhados com ≥ 40 px a 1080p", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 1080 });
  const nova = screens.find((x) => x.name === "Nova partida");
  if (!nova) throw new Error("tela Nova partida não definida");
  await open(page, nova, { get_settings: { ok: settings("C:/Karaoke", { tvMode: true }) } });
  const sizes = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLInputElement>("input[type=checkbox], input[type=radio]")].map((el) => {
      const r = el.getBoundingClientRect();
      return { type: el.type, w: Math.round(r.width), h: Math.round(r.height) };
    }),
  );
  console.log("[tv] controles:", JSON.stringify(sizes));
  expect(sizes.length).toBeGreaterThan(0);
  for (const s of sizes) expect(Math.min(s.w, s.h), `${s.type} ${s.w}×${s.h}`).toBeGreaterThanOrEqual(40);
});

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
      const base = s.spec?.get_settings && "ok" in s.spec.get_settings ? (s.spec.get_settings.ok as AppSettings) : settings("C:/Karaoke");
      await open(page, s, { get_settings: { ok: { ...base, reducedMotion: setting } } });
      expect(await longMotion(page)).toEqual([]);
    });
  }
}

test("controle negativo: sem redução de movimento, o Início anima (a checagem acima não é vazia)", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await open(page, screens[0], { get_settings: { ok: settings("C:/Karaoke", { reducedMotion: "off" }) } });
  expect((await longMotion(page)).length).toBeGreaterThan(0);
});
