// T-013-04 (SPEC-013): UltraStar import against the stateful fake (e2e/fake-backend.ts). The parser itself is
// scoring_engine::import_ultrastar (Agent 2, tested in Rust + fixtures in tests/fixtures/references/ultrastar/).
import { expect, test } from "@playwright/test";
import type { RefNote } from "../src/lib/ipc";
import { DUET_NOTICE, installFakeBackend } from "./fake-backend";
import { fullSpec, nav } from "./fixtures";
import { ipcCalls, mockTauri, setReply, useSyntheticVideo, withIpc } from "./tauri-mock";

/** basic.txt of the fixtures (SPEC-013 criterion: BPM 300, GAP 1000). */
const basic: RefNote[] = [
  { startMs: 1000, durMs: 200, midi: 60, lyric: "Lá" },
  { startMs: 1200, durMs: 200, midi: 62, lyric: "ri" },
  { startMs: 1900, durMs: 200, midi: 64, lyric: "la" },
];
const ultrastar = {
  "basic.txt": { notes: basic },
  "duet.txt": { notes: basic.slice(0, 2), duet: true },
  "invalid-truncated-line.txt": { line: 4, message: "a nota não tem altura" },
};

test("contrato: .txt vira referência UltraStar ativa; dueto traz o aviso; linha inválida dá invalid_reference com o número", async ({ page }) => {
  await installFakeBackend(page, [], { ultrastar });
  await mockTauri(page, fullSpec());
  await page.goto("/");
  const imp = (file: string) => withIpc(page, (ipc, f) => ipc.importReference("c3", `C:/Karaoke/${f}`, null), file);

  const ok = await imp("basic.txt");
  expect(ok).toMatchObject({ songId: "c3", source: "ultrastar", status: "experimental", revision: 1, offsetMs: 0, notes: basic });
  expect(ok).not.toHaveProperty("notice"); // skipped in the JSON when there is none
  expect(await withIpc(page, (ipc, s) => ipc.getReference(s), "c3")).toMatchObject({ source: "ultrastar" });
  expect(await imp("duet.txt")).toMatchObject({ notes: basic.slice(0, 2), notice: DUET_NOTICE });
  expect(await imp("invalid-truncated-line.txt")).toEqual({
    rejected: { code: "invalid_reference", message: "Referência inválida: arquivo UltraStar, linha 4: a nota não tem altura." },
  });
});

// ---------- UI (T-013-03) ----------

const ALIGN_HINT =
  "Arquivos UltraStar costumam seguir o tempo da gravação original, não o deste vídeo. Use Alinhar automaticamente antes de validar.";

async function openSongReference(page: import("@playwright/test").Page, pick: string) {
  await installFakeBackend(page, [], { ultrastar });
  const tracker = await mockTauri(page, fullSpec({ "plugin:dialog|open": { ok: `C:/Karaoke/${pick}` }, upload_song_audio: { ok: "tok" } }));
  await page.goto("/");
  await useSyntheticVideo(page, 1000, true);
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: "Referência de Canção da Manhã" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Canção da Manhã" })).toBeVisible();
  return tracker;
}

test("UI: importar UltraStar → selo UltraStar + sugestão de alinhar, que some ao Alinhar automaticamente", async ({ page }) => {
  const { pageErrors } = await openSongReference(page, "basic.txt");
  await page.getByRole("button", { name: "Importar referência" }).click();
  const [dialog] = await ipcCalls(page, "plugin:dialog|open");
  expect(JSON.stringify(dialog.args)).toContain('"extensions":["mid","midi","json","txt"]');
  for (const badge of ["UltraStar", "Referência experimental", "3 notas"]) await expect(page.getByText(badge, { exact: true })).toBeVisible();
  const hint = page.getByRole("status").filter({ hasText: "Arquivos UltraStar costumam seguir" });
  await expect(hint).toHaveText(ALIGN_HINT);
  expect(await ipcCalls(page, "list_midi_tracks")).toEqual([]); // .txt: no track choice
  await page.getByRole("button", { name: "Alinhar automaticamente" }).click();
  await expect(hint).toHaveCount(0);
  await expect(page.getByText(/Proposta: \+1200 ms/)).toBeVisible({ timeout: 15_000 }); // the fake's default proposal, after decoding the audio
  expect(pageErrors).toEqual([]);
});

test("UI: dueto UltraStar mostra o aviso; linha inválida mostra o erro com o número da linha e nada muda", async ({ page }) => {
  const { pageErrors } = await openSongReference(page, "duet.txt");
  await page.getByRole("button", { name: "Importar referência" }).click();
  await expect(page.getByText(DUET_NOTICE)).toBeVisible();
  await expect(page.getByText("2 notas", { exact: true })).toBeVisible();

  await setReply(page, "plugin:dialog|open", { ok: "C:/Karaoke/invalid-truncated-line.txt" });
  await page.getByRole("button", { name: "Importar outra" }).click();
  await expect(page.getByText("Referência inválida: arquivo UltraStar, linha 4: a nota não tem altura.")).toBeVisible();
  await expect(page.getByText("2 notas", { exact: true })).toBeVisible(); // the duet import is still the active one
  expect(pageErrors).toEqual([]);
});
