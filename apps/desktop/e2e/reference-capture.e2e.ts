// T-005-09: "Gravar cantando" (T-005-07, SPEC-005 R8) end to end against the stateful test-only fake (e2e/fake-backend.ts).
import { expect, test, type Page } from "@playwright/test";
import type { MelodyReference, RefNote } from "../src/lib/ipc";
import { installFakeBackend } from "./fake-backend";
import { fullSpec, nav } from "./fixtures";
import { ipcCalls, mockTauri, useSyntheticVideo } from "./tauri-mock";

/** What the "singer" produces: five notes inside the synthetic video. */
const sung: RefNote[] = [60, 62, 64, 62, 60].map((midi, i) => ({ startMs: 200 + i * 400, durMs: 350, midi, lyric: null }));
const imported: MelodyReference = {
  id: 1,
  songId: "c3",
  source: "json",
  status: "validated",
  revision: 2,
  offsetMs: 0,
  notes: [{ startMs: 0, durMs: 500, midi: 60, lyric: null }],
};

async function openReference(page: Page, title: string) {
  await nav(page, "Biblioteca");
  await page.getByRole("button", { name: `Referência de ${title}` }).click();
  await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
}

const panel = (page: Page) => page.getByRole("region", { name: "Gravar cantando" });

/** Opens the capture panel, starts, waits for the 3-2-1 countdown and the recording badge. */
async function record(page: Page, start = "Começar") {
  await panel(page).getByRole("button", { name: start }).click();
  for (const n of ["3", "2", "1"]) await expect(panel(page).getByText(n, { exact: true })).toBeVisible();
  await expect(panel(page).getByText("Gravando notas")).toBeVisible();
}

test("gravar cantando: música sem referência → 3-2-1 → gravando → Parar → notas cantadas experimentais → validar", async ({ page }) => {
  await installFakeBackend(page, [], { captures: [sung] });
  const { pageErrors } = await mockTauri(page, fullSpec());
  await page.goto("/");
  await useSyntheticVideo(page, 3000);

  await openReference(page, "Canção da Manhã");
  await expect(page.getByRole("heading", { name: "Sem referência" })).toBeVisible();
  await page.getByRole("button", { name: "Gravar cantando" }).click();
  await expect(panel(page).getByText("A referência atual fica no histórico")).toHaveCount(0); // nothing to replace
  await record(page);
  expect((await ipcCalls(page, "start_performance")).map((c) => c.args)).toEqual([
    { args: { songId: "a1", roundId: null, participantId: null, referenceCapture: true }, onTick: expect.anything() },
  ]);
  await panel(page).getByRole("button", { name: "Parar" }).click();

  await expect(page.getByText("Notas gravadas. Confira o alinhamento e valide.")).toBeVisible();
  await expect(panel(page)).toHaveCount(0);
  for (const badge of ["Referência experimental", "Cantada", "Revisão 1", "5 notas"]) await expect(page.getByText(badge, { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Gravar de novo" })).toBeVisible(); // header: the active one is sung
  await expect(page.getByRole("slider", { name: "Offset das notas" })).toHaveValue("0");

  await page.getByRole("button", { name: "Validar alinhamento" }).click();
  await expect(page.getByText("Referência validada", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Validar alinhamento" })).toBeDisabled();
  expect(await ipcCalls(page, "abort_performance")).toEqual([]);
  expect(pageErrors).toEqual([]);
});

test("gravar cantando: poucas notas mostram o motivo, nada é salvo, e Gravar de novo funciona", async ({ page }) => {
  await installFakeBackend(page, [], { captures: [sung.slice(0, 2), sung] });
  const { pageErrors } = await mockTauri(page, fullSpec());
  await page.goto("/");
  await useSyntheticVideo(page, 3000);

  await openReference(page, "Canção da Manhã");
  await page.getByRole("button", { name: "Gravar cantando" }).click();
  await record(page);
  await panel(page).getByRole("button", { name: "Parar" }).click();
  await expect(panel(page).getByText("Poucas notas detectadas: cante a melodia mais perto do microfone e tente de novo.")).toBeVisible();
  expect(await page.evaluate(() => window.__E2E_HANDLERS__?.get_reference({ songId: "a1" }))).toBeNull();

  await record(page, "Gravar de novo");
  await panel(page).getByRole("button", { name: "Parar" }).click();
  await expect(page.getByText("5 notas", { exact: true })).toBeVisible();
  await expect(page.getByText("Cantada", { exact: true })).toBeVisible();
  expect(pageErrors).toEqual([]);
});

test("gravar cantando: Cancelar durante a gravação aborta a captura e não salva nada", async ({ page }) => {
  await installFakeBackend(page, [], { captures: [sung] });
  const { pageErrors } = await mockTauri(page, fullSpec());
  await page.goto("/");
  await useSyntheticVideo(page, 3000);

  await openReference(page, "Canção da Manhã");
  await page.getByRole("button", { name: "Gravar cantando" }).click();
  await record(page);
  await panel(page).getByRole("button", { name: "Cancelar" }).click();

  await expect(panel(page)).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Sem referência" })).toBeVisible();
  expect(await ipcCalls(page, "abort_performance")).toHaveLength(1);
  expect(await ipcCalls(page, "finish_performance")).toEqual([]);

  // The fake keeps live.rs's rule: a capture runs alone (no round, participant, leak check or probe).
  const err = await page.evaluate(() => {
    try {
      return window.__E2E_HANDLERS__?.start_performance({ args: { songId: "a1", roundId: 7, participantId: null, referenceCapture: true } });
    } catch (e) {
      return e;
    }
  });
  expect(err).toMatchObject({ code: "invalid_input" });
  expect(pageErrors).toEqual([]);
});

test("gravar cantando sobre uma referência existente: aviso de histórico, a cantada vira a ativa e sobrevive à reabertura", async ({ page }) => {
  await installFakeBackend(page, [], { references: [imported], captures: [sung] });
  const { pageErrors } = await mockTauri(page, fullSpec());
  await page.goto("/");
  await useSyntheticVideo(page, 3000);

  await openReference(page, "Pôr do Sol em Óculos");
  for (const badge of ["Referência validada", "JSON", "Revisão 2"]) await expect(page.getByText(badge, { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Gravar cantando" }).click();
  await expect(panel(page).getByText("A referência atual fica no histórico; a nova nasce experimental.")).toBeVisible();
  await record(page);
  await panel(page).getByRole("button", { name: "Parar" }).click();
  for (const badge of ["Referência experimental", "Cantada", "Revisão 1"]) await expect(page.getByText(badge, { exact: true })).toBeVisible();

  // Reopen the app: the sung one is the active reference; the imported one stays in the history, inactive.
  await page.reload();
  await openReference(page, "Pôr do Sol em Óculos");
  await expect(page.getByText("Cantada", { exact: true })).toBeVisible();
  const db = await page.evaluate(() => JSON.parse(localStorage.getItem("e2e-fake-db") ?? "null"));
  expect(db.references.map((r: MelodyReference) => r.source)).toEqual(["json", "manual"]);
  expect(db.activeRefs.c3).not.toBe(imported.id);
  expect(pageErrors).toEqual([]);
});
