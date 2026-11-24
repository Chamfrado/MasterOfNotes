// T-012-05 (SPEC-012) against the stateful fake (e2e/fake-backend.ts):
// - contract: the commands driven through the app's real client (src/lib/ipc.ts, loaded by Vite in the page):
//   raw upload (Uint8Array body + song-id header), progress Channel, cancel;
// - UI: Ajustes → Referência automática; Referência → Gerar automaticamente / Alinhar automaticamente.
import { expect, test, type Page } from "@playwright/test";
import type { MelodyReference } from "../src/lib/ipc";
import { NO_VOICE_REASON, installFakeBackend, knownModel } from "./fake-backend";
import { fullSpec, nav } from "./fixtures";
import { ipcCalls, mockTauri, setReply, useSyntheticVideo, withIpc } from "./tauri-mock";

async function setup(page: Page, opts: Parameters<typeof installFakeBackend>[2] = {}) {
  await installFakeBackend(page, [], { generateStepMs: 20, ...opts });
  const tracker = await mockTauri(page, fullSpec());
  await page.goto("/");
  return tracker;
}

test("modelo: lista conhecidos, recusa sem aceite e arquivo desconhecido, instala, aparece nas settings e remove", async ({ page }) => {
  await setup(page);
  expect(await withIpc(page, (ipc) => ipc.listKnownSeparationModels(), null)).toEqual([knownModel]);
  expect(await withIpc(page, (ipc, f) => ipc.installSeparationModel(f, false), `C:/Downloads/${knownModel.fileName}`)).toMatchObject({
    rejected: { code: "terms_not_accepted", message: expect.stringContaining("Li e aceito os termos da fonte") },
  });
  expect(await withIpc(page, (ipc) => ipc.installSeparationModel("C:/Downloads/outro.onnx", true), null)).toMatchObject({
    rejected: { code: "model_unknown", message: expect.stringContaining("não é um modelo conhecido") },
  });
  expect(await withIpc(page, (ipc, f) => ipc.installSeparationModel(f, true), `C:\\Downloads\\${knownModel.fileName}`)).toMatchObject({
    id: knownModel.id,
    name: knownModel.name,
  });
  expect(await withIpc(page, (ipc) => ipc.getSettings(), null)).toMatchObject({ libraryFolder: "C:/Karaoke", separationModel: { id: knownModel.id } });
  await withIpc(page, (ipc) => ipc.removeSeparationModel(), null);
  expect(await withIpc(page, (ipc) => ipc.getSettings(), null)).toMatchObject({ separationModel: null });
});

test("gerar: upload raw com song-id, progresso por etapa, referência extraída ativa; sem voz não salva; cancelar", async ({ page }) => {
  const sung = [{ startMs: 500, durMs: 400, midi: 57, lyric: null }];
  await setup(page, { separationModel: { id: knownModel.id, name: knownModel.name, acceptedMs: 0 }, generated: [sung, "noVoice"] });

  const run = await withIpc(
    page,
    async (ipc, songId) => {
      const token = await ipc.uploadSongAudio(songId, new Float32Array(44_100)); // 1 s of silence
      const progress: string[] = [];
      const out = await ipc.generateReference(songId, token, (p) => progress.push(`${p.stage}:${p.fraction}`));
      return { token, progress, out, active: await ipc.getReference(songId) };
    },
    "c3",
  );
  expect(run).toMatchObject({
    token: "audio-1",
    progress: ["separating:0", "separating:0.5", "separating:1", "transcribing:0", "transcribing:0.5", "transcribing:1", "saving:0", "saving:0.5", "saving:1"],
    out: { reason: null, reference: { songId: "c3", source: "extracted", status: "experimental", notes: sung } },
  });
  expect(run).toMatchObject({ active: { id: (run as { out: { reference: { id: number } } }).out.reference.id } });
  const [upload] = await ipcCalls(page, "upload_song_audio");
  expect(upload.options).toEqual({ headers: { "song-id": "c3" } });

  // No voice (R3): nothing saved, the extracted reference stays active.
  const noVoice = await withIpc(
    page,
    async (ipc, songId) => {
      const token = await ipc.uploadSongAudio(songId, new Float32Array(441));
      const out = await ipc.generateReference(songId, token, () => {});
      return { out, active: await ipc.getReference(songId) };
    },
    "c3",
  );
  expect(noVoice).toMatchObject({ out: { reference: null, reason: NO_VOICE_REASON }, active: { source: "extracted" } });

  // Cancel mid-job; a stale token is refused.
  const cancelled = await withIpc(
    page,
    async (ipc, songId) => {
      const token = await ipc.uploadSongAudio(songId, new Float32Array(441));
      const job = ipc.generateReference(songId, token, (p) => {
        if (p.stage === "transcribing") void ipc.cancelGenerateReference();
      });
      const err = await job.catch((e: unknown) => e);
      const stale = await ipc.generateReference(songId, "audio-1", () => {}).catch((e: unknown) => e);
      return { err, stale };
    },
    "c3",
  );
  expect(cancelled).toMatchObject({ err: { code: "cancelled" }, stale: { code: "audio_token_invalid" } });
});

test("gerar exige modelo; alinhar propõe sem salvar e o offset aceito sobe a revisão", async ({ page }) => {
  const reference = { id: 1, songId: "c3", source: "midi" as const, status: "validated" as const, revision: 1, offsetMs: 0, notes: [{ startMs: 0, durMs: 500, midi: 60, lyric: null }] };
  await setup(page, { references: [reference] });
  const r = await withIpc(
    page,
    async (ipc, songId) => {
      const token = await ipc.uploadSongAudio(songId, new Float32Array(441));
      const noModel = await ipc.generateReference(songId, token, () => {}).catch((e: unknown) => e);
      const proposal = await ipc.autoAlignReference(1, token);
      const untouched = await ipc.getReference(songId);
      const saved = await ipc.setReferenceOffset(1, proposal.offsetMs);
      return { noModel, proposal, untouched, saved };
    },
    "c3",
  );
  expect(r).toMatchObject({
    noModel: { code: "model_not_installed", message: expect.stringContaining("Ajustes → Referência automática") },
    proposal: { offsetMs: 1200, confidence: 0.9 },
    untouched: { offsetMs: 0, status: "validated", revision: 1 },
    saved: { offsetMs: 1200, status: "experimental", revision: 2 },
  });
});

// ---------- UI ----------

const installed = { id: knownModel.id, name: knownModel.name, acceptedMs: Date.UTC(2026, 10, 20, 12, 0) };
const imported: MelodyReference = {
  id: 1,
  songId: "c3",
  source: "json",
  status: "validated",
  revision: 2,
  offsetMs: 0,
  notes: [{ startMs: 0, durMs: 500, midi: 60, lyric: null }],
};

/** App with the fake; `audio`: the synthetic video has an audio track (needed to decode, R6). */
async function openApp(page: Page, opts: Parameters<typeof installFakeBackend>[2] = {}, audio = true) {
  await installFakeBackend(page, [], { generateStepMs: 300, ...opts });
  const tracker = await mockTauri(page, fullSpec());
  await page.goto("/");
  await useSyntheticVideo(page, 1500, audio);
  return tracker;
}

async function openReference(page: Page, title: string) {
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: `Referência de ${title}` }).click();
  await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
}

const generatePanel = (page: Page) =>
  page.locator("div").filter({ has: page.getByRole("heading", { level: 2, name: "Gerar automaticamente" }) }).last();
const status = (page: Page) => generatePanel(page).getByRole("status");
/** Bytes of each upload_song_audio body (Uint8Array) and its song-id header. */
const uploads = (page: Page) =>
  page.evaluate(() =>
    (window.__E2E_CALLS__ ?? [])
      .filter((c) => c.cmd === "upload_song_audio")
      .map((c) => ({ bytes: (c.args as Uint8Array).byteLength, songId: c.options?.headers?.["song-id"] })),
  );

test("Ajustes: aceite obrigatório, arquivo desconhecido recusado, instala, sobrevive à reabertura e remove", async ({ page }) => {
  const { pageErrors } = await openApp(page);
  await setReply(page, "plugin:opener|open_url", { ok: null });
  await nav(page, "Ajustes");
  const section = page.locator("div").filter({ has: page.getByRole("heading", { level: 2, name: "Referência automática" }) }).last();
  await expect(section.getByText(knownModel.name)).toBeVisible();
  await expect(section.getByText(`Licença: ${knownModel.license}`)).toBeVisible();
  await expect(section.getByText(`Arquivo ${knownModel.fileName} · SHA-256 ${knownModel.id}`)).toBeVisible();
  await expect(section.getByText("Leia a página do projeto antes de baixar.")).toBeVisible();
  // Project page first (terms, credits), then the file itself: two distinct links.
  await section.getByRole("button", { name: "Abrir página do projeto" }).click();
  await section.getByRole("button", { name: "Baixar o arquivo do modelo" }).click();
  await expect
    .poll(async () => (await ipcCalls(page, "plugin:opener|open_url")).map((c) => (c.args as { url: string }).url))
    .toEqual([knownModel.pageUrl, knownModel.sourceUrl]);

  const choose = section.getByRole("button", { name: "Escolher arquivo baixado…" });
  const terms = section.getByRole("checkbox", { name: /Li e aceito os termos da fonte/ });
  await expect(choose).toBeDisabled(); // R1: terms first
  await terms.check();
  await setReply(page, "plugin:dialog|open", { ok: "C:/Downloads/outro-modelo.onnx" });
  await choose.click();
  await expect(page.getByText('"outro-modelo.onnx" não é um modelo conhecido: só os modelos da lista são aceitos.')).toBeVisible();

  await setReply(page, "plugin:dialog|open", { ok: `C:/Downloads/${knownModel.fileName}` });
  await choose.click();
  await expect(page.getByText(`Modelo ${knownModel.name} instalado.`)).toBeVisible();
  await expect(section.getByText("· instalado em 20/11/2026")).toBeVisible();

  await page.reload();
  await nav(page, "Ajustes");
  await expect(section.getByText("· instalado em 20/11/2026")).toBeVisible();
  await section.getByRole("button", { name: "Remover" }).click();
  await expect(page.getByText("Modelo removido.")).toBeVisible();
  await expect(terms).not.toBeChecked();
  await expect(choose).toBeDisabled();
  expect(pageErrors).toEqual([]);
});

test("Gerar sem modelo leva a Ajustes, com o foco no título da seção", async ({ page }) => {
  const { pageErrors } = await openApp(page);
  await openReference(page, "Canção da Manhã");
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await expect(page.getByText("Instale um modelo de separação de voz para gerar a referência automaticamente.")).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Referência automática" })).toBeFocused();
  expect(await ipcCalls(page, "generate_reference")).toEqual([]);
  expect(pageErrors).toEqual([]);
});

test("Gerar: etapas anunciadas sem mover o foco, referência Extraída experimental, validar; alinhar reaproveita o áudio", async ({ page }) => {
  // Each progress message must outlive expect's back-off polling (up to 1 s between checks); 9 messages ≈ 11 s.
  const { pageErrors } = await openApp(page, { separationModel: installed, generateStepMs: 1200 });
  await openReference(page, "Canção da Manhã");
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await expect(generatePanel(page).getByText("A referência atual vai para o histórico")).toHaveCount(0);
  await generatePanel(page).getByRole("button", { name: "Começar" }).click();

  await expect(status(page)).toContainText("Etapa 2 de 4: Separando a voz");
  const cancel = generatePanel(page).getByRole("button", { name: "Cancelar" });
  await expect(cancel).toBeFocused(); // "Começar" left: the focus goes to Cancelar, never to the body (F1)
  await expect(generatePanel(page).getByRole("progressbar", { name: "Separando a voz" })).toBeVisible();
  for (const text of ["Etapa 3 de 4: Transcrevendo (50%)", "Etapa 4 de 4: Salvando"]) {
    await expect(status(page)).toContainText(text, { timeout: 15_000 });
    await expect(cancel).toBeFocused(); // progress updates never move the focus
  }
  await expect(page.getByText(/^Referência gerada em .*Pode ter erros: confira sobre o vídeo e valide\.$/)).toBeVisible({ timeout: 15_000 });
  for (const badge of ["Referência experimental", "Extraída", "Revisão 1", "5 notas"]) await expect(page.getByText(badge, { exact: true })).toBeVisible();
  const sent = await uploads(page);
  expect(sent).toEqual([{ bytes: expect.any(Number), songId: "a1" }]);
  expect(sent[0].bytes % 4).toBe(0);
  // Decoded mono f32 samples. The length is not asserted: the synthetic track is recorded in real time and
  // comes out anywhere from ~0.2 to 1.5 s on a loaded machine.
  expect(sent[0].bytes).toBeGreaterThan(0);

  await page.getByRole("button", { name: "Validar alinhamento" }).click(); // R2: never validated by itself
  await expect(page.getByText("Referência validada", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Alinhar automaticamente" }).click();
  await expect(page.getByText(/Proposta: \+1200 ms \(confiança 90%\)/)).toBeVisible();
  expect(await uploads(page)).toHaveLength(1); // one upload per song (R6 cache)
  expect(pageErrors).toEqual([]);
});

test("Gerar: sem voz não salva e a referência anterior continua; Cancelar e sair da tela cancelam o trabalho", async ({ page }) => {
  const { pageErrors } = await openApp(page, { separationModel: installed, references: [imported], generated: ["noVoice"], generateStepMs: 500 });
  await openReference(page, "Pôr do Sol em Óculos");
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await expect(generatePanel(page).getByText("A referência atual vai para o histórico; a nova nasce experimental.")).toBeVisible();
  await generatePanel(page).getByRole("button", { name: "Começar" }).click();
  await expect(status(page)).toContainText(NO_VOICE_REASON);
  await expect(generatePanel(page).getByRole("button", { name: "Tentar de novo" })).toBeVisible();
  await generatePanel(page).getByRole("button", { name: "Cancelar" }).click();
  for (const badge of ["Referência validada", "JSON", "Revisão 2"]) await expect(page.getByText(badge, { exact: true })).toBeVisible();

  // Cancel in the middle.
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await generatePanel(page).getByRole("button", { name: "Começar" }).click();
  await expect(status(page)).toContainText("Separando a voz");
  await generatePanel(page).getByRole("button", { name: "Cancelar" }).click();
  await expect(page.getByText("Geração cancelada.")).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Gerar automaticamente" })).toHaveCount(0);
  await expect.poll(async () => (await ipcCalls(page, "cancel_generate_reference")).length).toBe(1);

  // Leaving the screen cancels too.
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await generatePanel(page).getByRole("button", { name: "Começar" }).click();
  await expect(status(page)).toContainText("Separando a voz");
  await nav(page, "Biblioteca");
  await expect.poll(async () => (await ipcCalls(page, "cancel_generate_reference")).length).toBe(2);
  await openReference(page, "Pôr do Sol em Óculos");
  await expect(page.getByText("JSON", { exact: true })).toBeVisible();
  expect(pageErrors).toEqual([]);
});

test("Alinhar: proposta com prévia, Descartar não salva, confiança baixa avisa, Aplicar salva (revisão + 1)", async ({ page }) => {
  const { pageErrors } = await openApp(page, {
    references: [{ ...imported, source: "midi" }],
    alignments: [
      { offsetMs: 1200, confidence: 0.92 },
      { offsetMs: -350, confidence: 0.3 },
    ],
  });
  await openReference(page, "Pôr do Sol em Óculos");
  const slider = page.getByRole("slider", { name: "Offset das notas" });
  await page.getByRole("button", { name: "Alinhar automaticamente" }).click();
  await expect(page.getByText(/Proposta: \+1200 ms \(confiança 92%\)\. A prévia já usa esse valor\./)).toBeVisible();
  await expect(slider).toHaveValue("1200");
  await expect(page.getByText("Confiança baixa: confira pelo vídeo antes de aplicar.")).toHaveCount(0);
  await page.getByRole("button", { name: "Descartar" }).click();
  await expect(slider).toHaveValue("0");
  expect(await ipcCalls(page, "set_reference_offset")).toEqual([]);

  await page.getByRole("button", { name: "Alinhar automaticamente" }).click();
  await expect(page.getByText(/Proposta: -350 ms \(confiança 30%\)/)).toBeVisible();
  await expect(page.getByText("Confiança baixa: confira pelo vídeo antes de aplicar.")).toBeVisible();
  await page.getByRole("button", { name: "Aplicar" }).click();
  await expect(page.getByText("Offset aplicado. Confira e valide de novo.")).toBeVisible();
  for (const badge of ["Referência experimental", "MIDI", "Revisão 3"]) await expect(page.getByText(badge, { exact: true })).toBeVisible();
  await expect(slider).toHaveValue("-350");
  expect(await uploads(page)).toHaveLength(1);
  expect(pageErrors).toEqual([]);
});

test("vídeo sem trilha de áudio: a geração falha com mensagem em pt-BR e permite tentar de novo", async ({ page }) => {
  const { pageErrors } = await openApp(page, { separationModel: installed }, false);
  await openReference(page, "Canção da Manhã");
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await generatePanel(page).getByRole("button", { name: "Começar" }).click();
  await expect(generatePanel(page).getByRole("button", { name: "Tentar de novo" })).toBeVisible();
  await expect(status(page)).toHaveText(
    "Não foi possível ler o áudio deste vídeo. O arquivo pode não ter som ou usar um formato de áudio não suportado.", // pt-BR, not the raw decoder error (F2)
  );
  expect(await ipcCalls(page, "generate_reference")).toEqual([]);
  expect(pageErrors).toEqual([]);
});


test("voz fraca (QA M2): −14 dB salva a referência com aviso de revisão; −8 dB substitui sem aviso", async ({ page }) => {
  const weak = { notes: [60, 62, 64].map((midi, i) => ({ startMs: 800 + i * 400, durMs: 350, midi, lyric: null })), voiceLevelDb: -14 };
  const clear = { notes: [60, 62, 64, 65].map((midi, i) => ({ startMs: 800 + i * 400, durMs: 350, midi, lyric: null })), voiceLevelDb: -8 };
  const { pageErrors } = await openApp(page, { separationModel: installed, generated: [weak, clear], generateStepMs: 50 });
  await openReference(page, "Canção da Manhã");
  const warning = page.getByRole("status").filter({ hasText: "A voz deste vídeo está fraca" });
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await generatePanel(page).getByRole("button", { name: "Começar" }).click();
  await expect(warning).toHaveText("A voz deste vídeo está fraca: a referência pode ter ficado incompleta. Revise as notas com atenção antes de validar.");
  await expect(page.getByText("3 notas", { exact: true })).toBeVisible();
  // Stored with the reference: still there after leaving the screen and coming back.
  await openReference(page, "Canção da Manhã");
  await expect(page.getByText("3 notas", { exact: true })).toBeVisible();
  await expect(warning).toHaveCount(1);

  // Generate again at −8 dB (above the −10 dB cut): the new reference replaces it, without the warning.
  await page.getByRole("button", { name: "Gerar automaticamente" }).click();
  await generatePanel(page).getByRole("button", { name: "Começar" }).click();
  await expect(page.getByText("4 notas", { exact: true })).toBeVisible();
  await expect(warning).toHaveCount(0);
  expect(pageErrors).toEqual([]);
});
