// T-012-05: SPEC-012 commands against the stateful fake, driven through the app's real client (src/lib/ipc.ts,
// loaded by Vite in the page): raw upload (Uint8Array body + song-id header), progress Channel, cancel.
// The UI flows (Ajustes → Referência automática; Referência → Gerar/Alinhar) join this file when T-012-04 lands.
import { expect, test, type Page } from "@playwright/test";
import { NO_VOICE_REASON, installFakeBackend, knownModel } from "./fake-backend";
import { fullSpec } from "./fixtures";
import { ipcCalls, mockTauri } from "./tauri-mock";

type Ipc = typeof import("../src/lib/ipc");

/** Runs `fn` in the page with the app's ipc module; returns its result or the rejected AppError. */
const withIpc = <T,>(page: Page, fn: (ipc: Ipc, arg: string) => Promise<T>, arg: string | null) =>
  page.evaluate(
    async ([src, arg]) => {
      const ipc = (await import("/src/lib/ipc.ts" as string)) as Ipc;
      return (new Function(`return ${src}`)() as typeof fn)(ipc, arg ?? "").catch((e: unknown) => ({ rejected: e }));
    },
    [fn.toString(), arg] as const,
  );

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
