// SPEC-011 T-011-02: every image, GIF and video of docs/user/ (IDs from PLANO-DE-MIDIA.md).
// Run: pnpm --filter desktop exec playwright test -c e2e/docs/playwright.docs.config.ts [-g "<página>"]
// Data is invented (Law 19); scores, notes and mic levels are injected by this script and illustrative (Law 03).
// The app's IPC is answered by the E2E mock (imported, never changed here).
import { expect, test, type Browser, type Page } from "@playwright/test";
import { existsSync, renameSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { AppSettings, MelodyReference, PerformanceOutcome, PerformanceRow, Song } from "../../src/lib/ipc";
import { installFakeBackend } from "../fake-backend";
import { fullSpec, settings, song } from "../fixtures";
import { mockTauri, setReply, type MockSpec } from "../tauri-mock";
import { FILES, MELODY_JSON, generateTestSong } from "../tools/test-song.ts";
import { BUDGET, MEDIA_DIR, checkBudget, gif, shot, totalMediaBytes } from "./media";

// ---------- example data ----------

const SONG_DIR = fileURLToPath(new URL("test-results/song/", import.meta.url));
const SONG_URL = `/e2e/docs/test-results/song/${FILES.karaoke}`;
const NOTES = MELODY_JSON.notes.map((n) => ({ ...n, lyric: n.lyric ?? null }));

const leakOk = { deviceId: null, videoVolume: 0.8, passed: true, atMs: Date.UTC(2026, 10, 19, 18) };
const ready = (extra: Partial<AppSettings> = {}) =>
  settings("C:/Karaoke", { noiseFloorDbfs: -58, updateConsentAsked: true, leakCheck: leakOk, ...extra });

const file = (id: string, name: string, title: string, artist: string | null, extra: Partial<Song> = {}) =>
  song(id, title, artist, { path: `C:/Karaoke/${name}`, fileName: name, extension: name.split(".").pop() ?? "", ...extra });

const LIBRARY: Song[] = [
  file("t1", FILES.karaoke, "Música de teste", "Master Of Notes", { referenceStatus: "validated", playable: "yes", durationMs: 31_000, sizeBytes: 1_900_000 }),
  file("a1", "cancao-da-manha.mp4", "Canção da Manhã", "Banda Aurora", { referenceStatus: "validated", playable: "yes", durationMs: 203_000, sizeBytes: 48_600_000 }),
  file("b2", "saudade-eletrica.mkv", "Saudade Elétrica", null, { durationMs: 245_000, sizeBytes: 61_200_000 }),
  file("c3", "por-do-sol-em-oculos.webm", "Pôr do Sol em Óculos", "Zé Mirante", { referenceStatus: "experimental", playable: "yes", durationMs: 188_000 }),
  file("e5", "estrela-cadente.mov", "Estrela Cadente", "Banda Aurora", { referenceStatus: "validated", playable: "yes", durationMs: 214_000 }),
  file("f6", "rua-da-chuva.mp4", "Rua da Chuva", "Lia Prado", { referenceStatus: "validated", playable: "yes", durationMs: 176_000 }),
  file("g7", "fita-antiga.avi", "Fita Antiga", null, { playable: "no", durationMs: null, sizeBytes: 702_000_000 }),
  file("h8", "valsa-do-quintal.mp4", "Valsa do Quintal", "Trio Varanda", { referenceStatus: "invalid", durationMs: 232_000 }),
];

const reference = (extra: Partial<MelodyReference> = {}): MelodyReference => ({
  id: 1,
  songId: "t1",
  source: "json",
  status: "validated",
  revision: 1,
  offsetMs: 0,
  notes: NOTES,
  ...extra,
});

const day = (d: number, h: number) => Date.UTC(2026, 10, d, h);
const performances: PerformanceRow[] = [
  { performanceId: 9, songId: "a1", songTitle: "Canção da Manhã", participantName: "Lia", score: 92.1, competitive: true, algorithmVersion: 2, referenceRevision: 1, difficulty: "normal", createdMs: day(18, 23), isCurrentKey: true },
  { performanceId: 8, songId: "a1", songTitle: "Canção da Manhã", participantName: null, score: 64.3, competitive: false, algorithmVersion: 2, referenceRevision: 1, difficulty: "normal", createdMs: day(18, 22), isCurrentKey: true },
  { performanceId: 7, songId: "a1", songTitle: "Canção da Manhã", participantName: "Rui", score: 71, competitive: true, algorithmVersion: 2, referenceRevision: 1, difficulty: "normal", createdMs: day(17, 21), isCurrentKey: true },
  { performanceId: 2, songId: "a1", songTitle: "Canção da Manhã", participantName: "Lia", score: 88.5, competitive: true, algorithmVersion: 1, referenceRevision: 1, difficulty: "normal", createdMs: day(2, 20), isCurrentKey: false },
];

const resultNoReference: PerformanceOutcome = {
  performanceId: null,
  competitive: false,
  result: null,
  exploratory: { voicedRatio: 0.72, pitchStability: 0.64 },
  personalBest: null,
  songRecord: null,
  reason: "Esta música não tem referência melódica: não há pontuação, só métricas de treino.",
};

const LEAK_REASON = "Faça o teste de vazamento do microfone (ou use fones) para pontuar no ranking.";
const resultTraining: PerformanceOutcome = {
  performanceId: 12,
  competitive: false,
  result: { score: 78.3, pitch: 0.82, rhythm: 0.7, sustain: 0.75, coverage: 0.84, notesTotal: 25, notesHit: 21, algorithmVersion: 3 },
  exploratory: null,
  personalBest: null,
  songRecord: null,
  reason: LEAK_REASON,
};

/** Everything the screens call, answered with the example data above. */
const BASE: MockSpec = {
  get_settings: { ok: ready() },
  update_settings: { ok: ready() },
  list_songs: { ok: LIBRARY },
  get_reference: { ok: reference() },
  list_participants: { ok: [{ id: 1, name: "Lia" }, { id: 2, name: "Rui" }, { id: 3, name: "Ana" }] },
  list_performances: { ok: performances },
  get_leaderboard: {
    ok: [
      { rank: 1, participantName: "Lia", score: 92.1, createdMs: day(18, 23), performanceId: 9 },
      { rank: 2, participantName: "Ana", score: 79.8, createdMs: day(15, 22), performanceId: 5 },
      { rank: 3, participantName: "Rui", score: 71, createdMs: day(17, 21), performanceId: 7 },
    ],
  },
  list_sessions: { ok: [] },
  "plugin:app|version": { ok: "0.1.0" },
  "plugin:resources|close": { ok: null },
  "plugin:updater|check": { ok: null },
};

// ---------- page setup ----------

declare global {
  interface Window {
    /** Sends `msg` on the Channel passed as `key` in the latest `cmd` call, as Rust would. */
    __docsPush?: (cmd: string, key: string, msg: unknown) => boolean;
    __docsStop?: () => void;
  }
}
type Internals = { invoke: (cmd: string, args: unknown, options?: unknown) => Promise<unknown>; runCallback: (id: number, data: unknown) => void };

type FakeOptions = NonNullable<Parameters<typeof installFakeBackend>[2]>;
async function open(
  page: Page,
  spec: MockSpec = {},
  opts: { fake?: number[]; fakeOptions?: FakeOptions; delays?: Record<string, number>; video?: string } = {},
) {
  if (opts.fake || opts.fakeOptions) await installFakeBackend(page, opts.fake ?? [], opts.fakeOptions);
  await mockTauri(page, fullSpec({ ...BASE, ...spec }));
  await page.addInitScript(
    ({ delays, videoUrl }) => {
      window.__E2E_VIDEO_URL__ = videoUrl;
      const tauri = (window as unknown as { __TAURI_INTERNALS__: Internals }).__TAURI_INTERNALS__;
      // Slow commands (scan, calibration) take real time in the app; keep that visible in the GIFs.
      const invoke = tauri.invoke;
      tauri.invoke = (cmd, args, options) => {
        const ms = delays[cmd];
        const p = invoke(cmd, args, options); // options: raw IPC headers (upload_song_audio)
        return ms ? p.then((v) => new Promise((r) => setTimeout(() => r(v), ms))) : p;
      };
      const next = new Map<number, number>();
      window.__docsPush = (cmd, key, msg) => {
        const call = (window.__E2E_CALLS__ ?? []).filter((c) => c.cmd === cmd).pop();
        const ch = (call?.args as Record<string, { id: number } | undefined> | undefined)?.[key];
        if (!ch) return false;
        const index = next.get(ch.id) ?? 0;
        next.set(ch.id, index + 1);
        tauri.runCallback(ch.id, { index, message: msg });
        return true;
      };
    },
    { delays: opts.delays ?? {}, videoUrl: opts.video ?? SONG_URL },
  );
  await page.goto("/");
}

const nav = (page: Page, label: string) => page.getByRole("navigation", { name: "Principal" }).getByRole("button", { name: label }).click();
const pause = (page: Page, ms: number) => page.waitForTimeout(ms);

/** Illustrative live mic level: singing a C4 with a little wobble (~30 frames/s). */
const feedMic = (page: Page) =>
  page.evaluate(() => {
    let n = 0;
    const id = setInterval(() => {
      n++;
      const singing = n % 50 > 10;
      window.__docsPush?.(
        "start_mic_monitor",
        "onFrame",
        singing
          ? { rmsDbfs: -20 - 4 * Math.sin(n / 4), f0Hz: 261.6 * (1 + 0.003 * Math.sin(n / 3)), confidence: 0.95, voiced: true, clipped: false }
          : { rmsDbfs: -54, f0Hz: null, confidence: 0.1, voiced: false, clipped: false },
      );
    }, 33);
    window.__docsStop = () => clearInterval(id);
  });

/** Illustrative live feedback following the reference under the video clock (5 ticks every 50 ms ≈ the real 100/s).
 * `leak`: the mic hears the music where the singer should be silent. */
const feedTicks = (page: Page, leak = false) =>
  page.evaluate(
    ({ notes, leak }) => {
      const v = document.querySelector("video");
      let n = 0;
      const id = setInterval(() => {
        const t = (v?.currentTime ?? 0) * 1000;
        const note = leak ? undefined : notes.find((x) => t >= x.startMs && t < x.startMs + x.durMs);
        for (let k = 0; k < 5; k++) {
          n++;
          const cents = note ? Math.round(24 * Math.sin(n / 23)) : null;
          const abs = Math.abs(cents ?? 0);
          window.__docsPush?.("start_performance", "onTick", {
            mic: { rmsDbfs: note || leak ? -21 : -58, f0Hz: note || leak ? 262 : null, confidence: 0.9, voiced: !!note || leak, clipped: false },
            feedback: note
              ? {
                  tMs: t,
                  targetMidi: note.midi,
                  sungMidi: note.midi + (cents ?? 0) / 100,
                  centsError: cents,
                  label: abs < 10 ? "excellent" : abs < 20 ? "veryGood" : "good",
                  runningScore: Math.round(780 + (t / 31_000) * 90) / 10,
                }
              : null,
          });
        }
      }, 50);
      window.__docsStop = () => clearInterval(id);
    },
    { notes: NOTES, leak },
  );

/** Library → Cantar → Começar → countdown → playing. */
async function startSinging(page: Page, title = "Música de teste") {
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: new RegExp(`^Cantar ${title}`) }).click();
}
const seekTo = (page: Page, s: number) =>
  page.evaluate((s) => {
    const v = document.querySelector("video");
    if (v) v.currentTime = s;
  }, s);

/** Records a fresh page as WebM (Playwright's own recorder) → docs/user/media/<id>.webm. */
async function video(browser: Browser, id: string, run: (page: Page) => Promise<void>) {
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    locale: "pt-BR",
    recordVideo: { dir: `${SONG_DIR}../video/`, size: { width: 960, height: 600 } },
  });
  const page = await context.newPage();
  await run(page);
  const recorded = page.video();
  await context.close();
  if (!recorded) throw new Error("vídeo não gravado");
  renameSync(await recorded.path(), `${MEDIA_DIR}${id}.webm`);
  checkBudget(`${id}.webm`);
}

// ---------- the test song (self-authored, Law 19), generated once ----------

test.beforeAll(async ({ browser }) => {
  if (existsSync(SONG_DIR + FILES.karaoke)) return;
  test.setTimeout(180_000);
  const page = await browser.newPage();
  await generateTestSong(page, SONG_DIR); // real time, ~1 min
  await page.close();
});

// ---------- 02 primeira execução ----------

test("02 primeira execução", async ({ page }) => {
  await open(page, { get_settings: { ok: settings(null) }, list_songs: { ok: [] } });
  await expect(page.getByText("Quer que o Master Of Notes verifique")).toBeVisible();
  await shot(page, "inicio-primeira-vez");
  await shot(page.getByRole("navigation", { name: "Principal" }), "menu-principal");

  await setReply(page, "get_settings", { ok: ready() });
  await nav(page, "Biblioteca");
  await nav(page, "Início");
  await expect(page.getByText("Calibrado para o ruído do ambiente.")).toBeVisible();
  await shot(page, "inicio-pronto");
});

// ---------- 03 biblioteca ----------

test("03 biblioteca", async ({ page }) => {
  await open(
    page,
    {
      get_settings: { ok: settings(null, { updateConsentAsked: true }) },
      list_songs: { ok: [] },
      "plugin:dialog|open": { ok: "C:/Karaoke" },
      set_library_folder: { ok: { found: 8, added: 8, updated: 0, removed: 0, skipped: 1, durationMs: 900 } },
    },
    { delays: { set_library_folder: 1500 } },
  );
  await nav(page, "Biblioteca");
  await expect(page.getByText("Comece escolhendo sua pasta de músicas")).toBeVisible();
  await shot(page, "biblioteca-vazia");

  await gif(page, "biblioteca-escolher-pasta", async () => {
    await pause(page, 800);
    await setReply(page, "list_songs", { ok: LIBRARY });
    await setReply(page, "get_settings", { ok: ready() });
    await page.getByRole("button", { name: "Escolher pasta" }).first().click();
    await expect(page.getByText("8 músicas encontradas (8 novas, 0 removidas)")).toBeVisible();
    await pause(page, 2000);
  });
  await page.mouse.move(0, 0);
  await shot(page, "biblioteca-grade");

  await gif(page, "biblioteca-busca", async () => {
    await pause(page, 600);
    await page.getByLabel("Buscar músicas").pressSequentially("cancao", { delay: 180 });
    await pause(page, 1500);
    await page.getByLabel("Buscar músicas").fill("");
    await page.getByLabel("Buscar músicas").pressSequentially("aurora", { delay: 180 });
    await pause(page, 1200);
  });
  await page.getByLabel("Buscar músicas").fill("");

  await page.getByRole("button", { name: "Lista" }).click();
  await shot(page, "biblioteca-lista");
  await page.getByRole("button", { name: "Grade" }).click();

  await page.getByRole("button", { name: "Editar Canção da Manhã" }).click();
  await expect(page.getByRole("dialog", { name: "Editar música" })).toBeVisible();
  await shot(page, "biblioteca-editar");
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Detalhes de Canção da Manhã" }).click();
  await page.getByLabel("Recorde pessoal de").selectOption("Lia");
  await expect(page.getByText("Lia: 92,1 pontos.")).toBeVisible();
  await page.mouse.move(0, 0);
  await shot(page, "detalhes-musica", { fullPage: true });

  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Detalhes de Fita Antiga" }).click();
  await shot(page.getByRole("region", { name: "Reprodução" }), "detalhes-incompativel");
});

// ---------- 04 microfone ----------

test("04 microfone", async ({ page }) => {
  test.setTimeout(240_000);
  await open(page, { get_settings: { ok: ready({ noiseFloorDbfs: null, leakCheck: null }) } }, { delays: { calibrate_noise: 3000 } });
  await nav(page, "Microfone");
  await feedMic(page);
  await expect(page.getByText("Voz detectada")).toBeVisible();
  await gif(page, "microfone-voz-detectada", () => pause(page, 4000), 0);
  await page.evaluate(() => window.__docsStop?.());
  await page.evaluate(() =>
    window.__docsPush?.("start_mic_monitor", "onFrame", { rmsDbfs: -19, f0Hz: 261.6, confidence: 0.95, voiced: true, clipped: false }),
  );
  await shot(page, "microfone-tela", { fullPage: true });

  const calibration = page.getByRole("heading", { name: "Calibração de ruído" }).locator("..");
  await gif(page, "microfone-calibrar", async () => {
    await pause(page, 600);
    await page.getByRole("button", { name: "Calibrar", exact: true }).click();
    await expect(page.getByText("Ambiente bom para cantar.")).toBeVisible({ timeout: 10_000 });
    await pause(page, 1200);
  });
  await setReply(page, "calibrate_noise", { ok: { noiseFloorDbfs: -38, clippedRatio: 0.04, issues: ["noisy", "clipping"] } });
  await page.getByRole("button", { name: "Calibrar de novo" }).click();
  await expect(page.getByText("O microfone está saturando")).toBeVisible({ timeout: 10_000 });
  await shot(calibration, "microfone-calibrar-problemas");

  await setReply(page, "finish_performance", {
    ok: { ...resultNoReference, exploratory: null, latency: { offsetMs: 142, jitterMs: 9, detected: 8, expected: 8, passed: true, reason: null } },
  });
  await page.getByRole("button", { name: "Medir latência" }).click();
  await expect(page.getByText(/Latência medida/)).toBeVisible({ timeout: 20_000 });
  await shot(page.getByRole("heading", { name: "Medir automaticamente" }).locator(".."), "microfone-latencia-auto");

  const leakCard = page.getByRole("heading", { name: "Teste de vazamento" }).locator("../..");
  await setReply(page, "finish_performance", {
    ok: {
      ...resultNoReference,
      exploratory: null,
      leak: { passed: false, guideScore: 0.62, reason: "O microfone está captando a voz ou a melodia da música. Use fones de ouvido ou abaixe o volume." },
    },
  });
  await setReply(page, "get_settings", { ok: ready({ leakCheck: { ...leakOk, passed: false } }) });
  await page.getByRole("button", { name: "Fazer teste" }).click();
  await expect(page.getByText("O microfone está captando a voz ou a melodia da música.", { exact: false })).toBeVisible({ timeout: 25_000 });
  await shot(leakCard, "microfone-vazamento-reprovado");

  await setReply(page, "finish_performance", { ok: { ...resultNoReference, exploratory: null, leak: { passed: true, guideScore: 0.03, reason: null } } });
  await setReply(page, "get_settings", { ok: ready() });
  await page.getByRole("button", { name: "Testar de novo" }).click();
  await expect(page.getByText(/^Aprovado: partidas/)).toBeVisible({ timeout: 25_000 });
  await shot(leakCard, "microfone-vazamento-aprovado");
});

// ---------- 05 referência melódica ----------

test("05 referência melódica", async ({ page }) => {
  await open(page, {
    get_reference: { ok: null },
    "plugin:dialog|open": { ok: "C:/Karaoke/referencias/saudade-eletrica.mid" },
    list_midi_tracks: {
      ok: [
        { index: 0, name: "Bateria", noteCount: 412 },
        { index: 1, name: "Baixo", noteCount: 236 },
        { index: 2, name: "Voz", noteCount: 188 },
        { index: 3, name: null, noteCount: 57 },
      ],
    },
  });
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Referência de Saudade Elétrica" }).click();
  await expect(page.getByRole("heading", { name: "Sem referência" })).toBeVisible();
  await shot(page, "referencia-vazia");
  await page.getByRole("button", { name: "Importar referência" }).click();
  await expect(page.getByRole("dialog", { name: "Qual trilha é a voz?" })).toBeVisible();
  await shot(page, "referencia-trilha");
});

test("05 referência melódica — alinhar e validar", async ({ page }) => {
  await open(page, {
    get_reference: { ok: reference({ status: "experimental", offsetMs: -300 }) },
    set_reference_offset: { ok: reference({ status: "experimental", offsetMs: 0, revision: 1 }) },
    set_reference_status: { ok: reference() },
  });
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Referência de Música de teste" }).click();
  await page.evaluate(async () => {
    const v = document.querySelector("video");
    if (!v) return;
    v.currentTime = 1.5;
    await v.play();
  });
  await gif(page, "referencia-alinhar", async () => {
    await pause(page, 1500);
    for (let i = 0; i < 3; i++) {
      await page.getByRole("button", { name: "+100 ms" }).click();
      await pause(page, 900);
    }
    await pause(page, 1500);
    await page.getByRole("button", { name: "Salvar offset" }).click();
    await pause(page, 1500);
  });
  await page.getByRole("button", { name: "Validar alinhamento" }).click();
  await expect(page.getByText("Referência validada: partidas valem ranking.")).toBeVisible();
  await page.evaluate(() => {
    document.querySelector("video")?.pause();
    window.scrollTo(0, 0);
  });
  await shot(page, "referencia-validar", { fullPage: true });
});

test("05 referência melódica — gravar cantando", async ({ page }) => {
  const sung = reference({ id: 2, source: "manual", status: "experimental" });
  await open(
    page,
    { get_reference: { ok: null }, finish_performance: { ok: { ...resultNoReference, exploratory: null, reason: null, capturedReference: sung } } },
    { delays: { finish_performance: 1200 } },
  );
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Referência de Música de teste" }).click();
  await page.getByRole("button", { name: "Gravar cantando" }).click();
  await expect(page.getByRole("button", { name: "Começar" })).toBeVisible();
  await shot(page, "referencia-cantar");
  await gif(page, "referencia-cantar", async () => {
    await pause(page, 800);
    await page.getByRole("button", { name: "Começar" }).click();
    await expect(page.getByText("Gravando notas")).toBeVisible({ timeout: 6000 });
    await feedTicks(page);
    await pause(page, 4500);
    await page.evaluate(() => window.__docsStop?.());
    await page.getByRole("button", { name: "Parar" }).click();
    await expect(page.getByText("Cantada", { exact: true })).toBeVisible({ timeout: 6000 });
    await pause(page, 1500);
  });
});

/** song_analysis::known_models() (crates/song-analysis/src/lib.rs), as the Ajustes screen shows it. */
const UVR_MODEL = {
  id: "534b2070fcc7df514b13ef660dc8cbb328679c2374d04354a5c42bb14ecce111",
  name: "UVR-MDX-NET Voc_FT",
  fileName: "UVR-MDX-NET-Voc_FT.onnx",
  sizeBytes: 66_762_490,
  pageUrl: "https://github.com/Anjok07/ultimatevocalremovergui",
  sourceUrl: "https://github.com/TRvlvr/model_repo/releases/download/all_public_uvr_models/UVR-MDX-NET-Voc_FT.onnx",
  license:
    "Modelo do projeto Ultimate Vocal Remover (UVR). Os autores não declararam uma licença para os pesos do modelo; o código do UVR é MIT e os autores pedem crédito. Não sabemos se o uso comercial é permitido. O Master Of Notes não distribui este arquivo: você o baixa da fonte original e o uso é de sua responsabilidade.",
};

test("05 referência melódica — gerar e alinhar automaticamente", async ({ page }) => {
  test.setTimeout(240_000);
  await open(
    page,
    { "plugin:dialog|open": { ok: `C:/Downloads/${UVR_MODEL.fileName}` } },
    {
      fakeOptions: {
        knownModels: [UVR_MODEL],
        separationModel: null,
        generated: [NOTES, "noVoice"],
        generateStepMs: 450,
        alignments: [{ offsetMs: 40, confidence: 0.92 }],
      },
      video: SONG_URL.replace(FILES.karaoke, FILES.guide), // generating needs the singer's voice in the video
    },
  );
  await nav(page, "Ajustes");
  const section = page.getByRole("heading", { name: "Referência automática" }).locator("..");
  await expect(section.getByText(UVR_MODEL.name)).toBeVisible();
  await shot(section, "ajustes-modelo");
  await section.getByRole("checkbox").check();
  await section.getByRole("button", { name: "Escolher arquivo baixado…" }).click();
  await expect(section.getByText(/instalado em/)).toBeVisible();

  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Referência de Música de teste" }).click();
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await expect(page.getByRole("button", { name: "Começar" })).toBeVisible();
  await gif(page, "referencia-gerar", async () => {
    await pause(page, 1200);
    await page.getByRole("button", { name: "Começar" }).click();
    await expect(page.getByText("Extraída", { exact: true })).toBeVisible({ timeout: 30_000 });
    await pause(page, 2000);
  });

  await page.getByRole("button", { name: "Alinhar automaticamente" }).click();
  await expect(page.getByText(/^Proposta:/)).toBeVisible({ timeout: 30_000 });
  await page.evaluate(() => window.scrollTo(0, 0));
  await shot(page, "referencia-alinhar-auto");

  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Referência de Saudade Elétrica" }).click();
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await page.getByRole("button", { name: "Começar" }).click();
  const panel = page.getByRole("heading", { name: "Gerar automaticamente" }).locator("..");
  await expect(panel.getByText("Não encontramos uma voz clara neste vídeo", { exact: false })).toBeVisible({ timeout: 30_000 });
  await shot(panel, "referencia-sem-voz");
});

test("05 referência melódica — UltraStar", async ({ page }) => {
  const file = "musica-de-teste.txt";
  await open(
    page,
    { "plugin:dialog|open": { ok: `C:/Karaoke/referencias/${file}` } },
    // As an UltraStar file timed to the original recording would land on this video: 800 ms late (illustrative).
    { fakeOptions: { ultrastar: { [file]: { notes: NOTES.map((n) => ({ ...n, startMs: n.startMs + 800 })), duet: true } } } },
  );
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Referência de Música de teste" }).click();
  await page.getByRole("button", { name: "Importar referência" }).click();
  await expect(page.getByText("Arquivos UltraStar costumam seguir o tempo da gravação original", { exact: false })).toBeVisible();
  await expect(page.getByText("UltraStar", { exact: true })).toBeVisible();
  await expect(page.getByText("Arquivo de dueto: importamos a parte do primeiro cantor.")).toBeVisible();
  await page.evaluate(() => window.scrollTo(0, 0));
  await shot(page, "referencia-ultrastar");
});

// ---------- 06 cantar ----------

test("06 cantar", async ({ page }) => {
  test.setTimeout(240_000);
  await open(page);
  await gif(page, "cantar-comecar", async () => {
    await startSinging(page);
    await page.getByRole("button", { name: "Começar", exact: true }).click();
    await feedTicks(page);
    await pause(page, 5500);
  });

  await pause(page, 1500);
  await page.mouse.move(640, 300);
  await shot(page, "cantar-painel-completo");

  await page.keyboard.press(" ");
  await expect(page.getByRole("button", { name: "Continuar", exact: true })).toBeVisible();
  await shot(page.locator("video").locator("..").locator("div.absolute.inset-x-0.bottom-0"), "cantar-controles");
  await page.keyboard.press(" ");

  await gif(page, "cantar-painel-modos", async () => {
    for (let i = 0; i < 3; i++) {
      await pause(page, 1300);
      await page.keyboard.press("h");
    }
    await pause(page, 1000);
  });

  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("dialog", { name: "Pular trecho?" })).toBeVisible();
  await shot(page, "cantar-pular-trecho");
  await page.keyboard.press("Escape"); // = "Continuar valendo"; a click races the 100/s re-renders

  await page.evaluate(() => window.__docsStop?.());
  await seekTo(page, 30.2);
  await expect(page.getByRole("heading", { level: 1, name: "Resultado" })).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText("Novo recorde pessoal!")).toBeVisible();
  await shot(page, "resultado");
});

test("06 cantar — treino, vazamento e sem referência", async ({ page }) => {
  await open(page, {
    get_settings: { ok: ready({ leakCheck: null }) },
    start_performance: { ok: { competitive: false, reason: LEAK_REASON } },
  });
  await startSinging(page);
  await expect(page.getByRole("button", { name: "Começar como treino" })).toBeVisible();
  await shot(page, "cantar-treino-vazamento");

  await page.getByRole("button", { name: "Começar como treino" }).click();
  await feedTicks(page, true);
  await expect(page.getByText("O microfone está captando a música — use fones de ouvido.")).toBeVisible({ timeout: 15_000 });
  await shot(page.getByText("O microfone está captando a música — use fones de ouvido."), "cantar-aviso-vazamento");
  await page.evaluate(() => window.__docsStop?.());
  await setReply(page, "finish_performance", { ok: resultTraining });
  await seekTo(page, 30.2);
  await expect(page.getByRole("button", { name: "Fazer teste de vazamento" })).toBeVisible({ timeout: 15_000 });
  await shot(page, "resultado-treino");

  await setReply(page, "start_performance", {
    ok: { competitive: false, reason: "Esta música não tem referência melódica: não há pontuação, só métricas de treino." },
  });
  await setReply(page, "finish_performance", { ok: resultNoReference });
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: /^Cantar Saudade Elétrica/ }).click();
  await page.getByRole("button", { name: "Começar", exact: true }).click();
  await pause(page, 3500);
  await seekTo(page, 30.2);
  await expect(page.getByText("Métricas exploratórias")).toBeVisible({ timeout: 15_000 });
  await shot(page, "resultado-sem-referencia");
});

test("06 cantar — vídeo do fluxo completo", async ({ browser }) => {
  test.setTimeout(180_000);
  await video(browser, "cantar-fluxo-completo", async (page) => {
    await open(page);
    await startSinging(page);
    await pause(page, 800);
    await page.getByRole("button", { name: "Começar", exact: true }).click();
    await feedTicks(page);
    await expect(page.getByRole("heading", { level: 1, name: "Resultado" })).toBeVisible({ timeout: 60_000 });
    await pause(page, 3000);
  });
});

// ---------- 08 modos de jogo ----------

/** Sings the current round fast: start, jump near the end (not a user seek, so no practice), back to the session. */
async function singRound(page: Page) {
  await page.getByRole("button", { name: "Cantar", exact: true }).click();
  await page.getByRole("button", { name: "Começar", exact: true }).click();
  await pause(page, 3500);
  await seekTo(page, 30.5);
  await expect(page.getByRole("heading", { level: 1, name: "Resultado" })).toBeVisible({ timeout: 15_000 });
  await page.getByRole("button", { name: "Voltar para a partida" }).click();
}

test("08 modos de jogo, 09 histórico", async ({ page }) => {
  test.setTimeout(300_000);
  await open(page, {}, { fake: [86.2, 78.4, 91.0, 69.5, 83.1, 88.7, 84.0, 79.5] });

  await nav(page, "Jogar");
  await page.getByRole("button", { name: "Participantes" }).click();
  for (const name of ["Lia", "Rui", "Ana", "Bia"]) {
    await page.getByLabel("Novo cantor").fill(name);
    await page.getByLabel("Novo cantor").press("Enter");
  }
  await expect(page.getByRole("list", { name: "Cantores" }).getByRole("listitem")).toHaveCount(4);
  await page.getByLabel("Novo cantor").blur();
  await shot(page, "participantes");
  await page.keyboard.press("Escape");

  // Party: 3 singers, 3 songs (manual order) → 3 rounds.
  await page.getByRole("button", { name: /^Festa/ }).click();
  await gif(page, "festa-criar", async () => {
    await pause(page, 600);
    await page.getByLabel("Nome da festa").pressSequentially("Festa de sexta", { delay: 60 });
    await page.getByLabel("Na ordem em que escolhi cantores e músicas").check();
    for (const name of ["Lia", "Rui", "Ana"]) {
      await page.getByRole("button", { name, exact: true }).click();
      await pause(page, 300);
    }
    for (const title of ["Canção da Manhã", "Estrela Cadente", "Rua da Chuva"]) {
      await page.getByRole("checkbox", { name: new RegExp(`^${title}`) }).check();
      await pause(page, 300);
    }
    await pause(page, 800);
    await page.getByRole("button", { name: "Criar festa" }).click();
    await expect(page.getByText("Rodada 1 de 3")).toBeVisible();
    await pause(page, 1500);
  });
  await singRound(page);
  await singRound(page);
  await page.mouse.move(0, 0);
  await shot(page, "festa-rodada");
  await singRound(page);
  await page.getByRole("button", { name: "Encerrar" }).click();
  await expect(page.getByText("Vencedor da festa")).toBeVisible();
  await shot(page, "festa-vencedor");

  // Duel.
  await nav(page, "Jogar");
  await page.getByRole("button", { name: /^Duelo/ }).click();
  await page.getByRole("button", { name: "Lia", exact: true }).click();
  await page.getByRole("button", { name: "Rui", exact: true }).click();
  await page.getByRole("radio", { name: /^Canção da Manhã/ }).check();
  await page.getByRole("button", { name: "Criar duelo" }).click();
  await singRound(page);
  await singRound(page);
  await page.getByRole("button", { name: "Encerrar" }).click();
  await expect(page.getByText(/venceu o duelo por/)).toBeVisible();
  await shot(page, "duelo");

  // Challenge: left running (2 of 3) so "Continuar de onde parou" shows on Jogar.
  await nav(page, "Jogar");
  await page.getByRole("button", { name: /^Desafio/ }).click();
  await page.getByLabel("Nome do desafio (opcional)").fill("Maratona da Ana");
  await page.getByRole("button", { name: "Ana", exact: true }).click();
  for (const title of ["Música de teste", "Estrela Cadente", "Rua da Chuva"]) {
    await page.getByRole("checkbox", { name: new RegExp(`^${title}`) }).check();
  }
  await page.getByRole("button", { name: "Criar desafio" }).click();
  await singRound(page);
  await singRound(page);
  await page.mouse.move(0, 0);
  await shot(page, "desafio");

  await nav(page, "Jogar");
  await expect(page.getByText("Continuar de onde parou")).toBeVisible();
  await shot(page, "jogar-modos");
  await page.getByRole("button", { name: "Histórico" }).click();
  await shot(page, "historico");
});

// ---------- 09 ranking ----------

test("09 ranking", async ({ page }) => {
  await open(page);
  await nav(page, "Ranking");
  await page.getByLabel("Música").selectOption({ label: "Canção da Manhã" });
  await expect(page.getByRole("table")).toBeVisible();
  await shot(page, "ranking");
});

// ---------- 10 ajustes e Modo TV ----------

test("10 ajustes e modo TV", async ({ page }) => {
  await open(page, { get_settings: { ok: ready({ reducedMotion: "system" }) } }); // the real default
  await nav(page, "Ajustes");
  await shot(page, "configuracoes", { fullPage: true });

  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Detalhes de Música de teste" }).focus();
  await gif(page, "navegacao-setas", async () => {
    for (const key of ["ArrowRight", "ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp"]) {
      await pause(page, 700);
      await page.keyboard.press(key);
    }
    await pause(page, 700);
    await page.keyboard.press("Enter");
    await pause(page, 1500);
  });
});

test("10 modo TV", async ({ page }) => {
  await open(page, { get_settings: { ok: ready({ tvMode: true }) } });
  await expect(page.locator("html[data-tv=on]")).toHaveCount(1);
  await shot(page, "modo-tv");
});

// ---------- 11 atualizações ----------

test("11 atualizações", async ({ page }) => {
  await open(page, {
    "plugin:updater|check": {
      ok: {
        rid: 1,
        currentVersion: "0.1.0",
        version: "0.2.0",
        date: "2026-12-01T12:00:00Z",
        body: "Exemplo de notas da versão:\n• Novo modo de jogo\n• Correções no microfone",
        rawJson: {},
      },
    },
    "plugin:updater|download": { ok: 2 },
  });
  await nav(page, "Ajustes");
  await page.getByRole("button", { name: "Verificar agora" }).click();
  await expect(page.getByRole("heading", { name: "Versão 0.2.0 disponível" })).toBeVisible();
  await shot(page, "atualizacao-disponivel");
  await page.getByRole("button", { name: "Baixar atualização" }).click();
  await expect(page.getByRole("dialog", { name: "Instalar agora?" })).toBeVisible();
  await shot(page, "atualizacao-instalar");
  await page.getByRole("button", { name: "Mais tarde" }).click();

  await setReply(page, "plugin:updater|check", { ok: null });
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Verificar agora" }).click();
  await expect(page.getByText("Você já está na versão mais recente.")).toBeVisible();
  await shot(page.getByText("Você já está na versão mais recente.").locator(".."), "atualizacao-em-dia");
});

test("11 atualizações — sem versão normal", async ({ page }) => {
  await open(page, { "plugin:updater|check": { err: { code: "updater", message: "Could not fetch a valid release JSON from the remote" } } });
  await nav(page, "Ajustes");
  await page.getByRole("button", { name: "Verificar agora" }).click();
  const msg = page.getByText("Nenhuma versão nova pelo canal de atualizações automáticas.");
  await expect(msg).toBeVisible();
  await shot(msg.locator(".."), "atualizacao-sem-versao");
});

// ---------- budget ----------

test.afterAll(() => {
  const total = totalMediaBytes();
  console.log(`Mídia do guia: ${(total / 1_048_576).toFixed(1)} MB`);
  expect(total, "mídia total passa de 40 MB (SPEC-011 R4)").toBeLessThanOrEqual(BUDGET.total);
});
