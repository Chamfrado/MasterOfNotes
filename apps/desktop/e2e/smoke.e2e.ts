// UI smoke flows against the Vite dev server with a test-only IPC mock (docs/testing/strategy.md).
import { expect, test, type Page } from "@playwright/test";
import type { AppSettings, Song } from "../src/lib/ipc";
import { ipcCalls, mockTauri, setReply, type MockSpec } from "./tauri-mock";

const settings = (libraryFolder: string | null): AppSettings => ({
  libraryFolder,
  recursiveScan: true,
  inputDeviceId: null,
  noiseFloorDbfs: null,
  latencyOffsetMs: 0,
  difficulty: "normal",
  videoVolume: 0.8,
  reducedMotion: "on",
  feedbackOverlay: "full",
  autoCheckUpdates: false,
});

// Invented titles (Law 19): no third-party content in fixtures.
const song = (id: string, title: string, artist: string | null, extra: Partial<Song> = {}): Song => ({
  id,
  path: `C:/Karaoke/${id}.mp4`,
  fileName: `${id}.mp4`,
  title,
  artist,
  extension: "mp4",
  sizeBytes: 1_000_000,
  durationMs: 185_000,
  playable: "unknown",
  available: true,
  referenceStatus: "none",
  ...extra,
});
const songs: Song[] = [
  song("a1", "Canção da Manhã", "Banda Aurora"),
  song("b2", "Saudade Elétrica", null),
  song("c3", "Pôr do Sol em Óculos", "Zé Mirante", { referenceStatus: "validated" }),
  song("d4", "Arquivo Sumido", null, { available: false }),
];
const appError = (message: string) => ({ err: { code: "TEST", message } });

async function openLibrary(page: Page, spec: MockSpec) {
  const tracker = await mockTauri(page, { get_settings: { ok: settings("C:/Karaoke") }, ...spec });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Principal" }).getByRole("button", { name: "Biblioteca" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Biblioteca" })).toBeVisible();
  return tracker;
}

test("biblioteca vazia sem pasta mostra o primeiro passo", async ({ page }) => {
  const { pageErrors } = await openLibrary(page, { get_settings: { ok: settings(null) }, list_songs: { ok: [] } });
  await expect(page.getByRole("heading", { name: "Comece escolhendo sua pasta de músicas" })).toBeVisible();
  await expect(page.getByText("Nenhuma pasta escolhida")).toBeVisible();
  await expect(page.getByRole("button", { name: "Escolher pasta" })).toHaveCount(2);
  expect(pageErrors).toEqual([]);
});

test("pasta sem vídeos mostra formatos aceitos", async ({ page }) => {
  const { pageErrors } = await openLibrary(page, { list_songs: { ok: [] } });
  await expect(page.getByRole("heading", { name: "Nenhum vídeo encontrado" })).toBeVisible();
  await expect(page.getByText("Formatos aceitos: MP4, MKV, WEBM, MOV e AVI.")).toBeVisible();
  expect(pageErrors).toEqual([]);
});

test("lista só disponíveis e busca ignora acento e caixa", async ({ page }) => {
  const { pageErrors } = await openLibrary(page, { list_songs: { ok: songs } });
  await expect(page.getByText("3 músicas", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: /^Cantar / })).toHaveCount(3);
  await expect(page.getByText("Arquivo Sumido")).toHaveCount(0);

  const search = page.getByRole("searchbox", { name: "Buscar músicas" });
  await search.fill("cancao");
  await expect(page.getByText("1 de 3 músicas")).toBeVisible();
  await expect(page.getByRole("button", { name: "Cantar Canção da Manhã, de Banda Aurora" })).toBeVisible();

  await search.fill("ELETRICA");
  await expect(page.getByRole("button", { name: "Cantar Saudade Elétrica" })).toBeVisible();
  await expect(page.getByText("1 de 3 músicas")).toBeVisible();

  await search.fill("ze mirante"); // artist field, accents and case folded
  await expect(page.getByRole("button", { name: /^Cantar Pôr do Sol em Óculos/ })).toBeVisible();

  await search.fill("xyz");
  await expect(page.getByText("Nada encontrado para “xyz”.")).toBeVisible();
  expect(pageErrors).toEqual([]);
});

test("falha ao listar a biblioteca mostra o erro do backend e permite tentar de novo", async ({ page }) => {
  const { pageErrors } = await openLibrary(page, {
    list_songs: appError("Não foi possível ler o banco de dados."),
  });
  const alert = page.getByRole("alert");
  await expect(alert.getByRole("heading", { name: "Não foi possível carregar a biblioteca" })).toBeVisible();
  await expect(alert.getByText("Não foi possível ler o banco de dados.")).toBeVisible();
  await setReply(page, "list_songs", { ok: songs });
  await alert.getByRole("button", { name: "Tentar de novo" }).click();
  await expect(page.getByText("3 músicas", { exact: true })).toBeVisible();
  expect(pageErrors).toEqual([]);
});

test("player mostra o erro do backend quando a música não carrega e Esc volta", async ({ page }) => {
  const { pageErrors } = await openLibrary(page, { list_songs: { ok: songs } });
  await setReply(page, "list_songs", appError("A biblioteca está indisponível no momento."));
  await page.getByRole("button", { name: "Cantar Saudade Elétrica" }).click();
  await expect(page.getByRole("alert").getByText("A biblioteca está indisponível no momento.")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("heading", { level: 1, name: "Biblioteca" })).toBeVisible();
  expect(pageErrors).toEqual([]);
});

test("player com vídeo ilegível mostra o aviso de codec e marca a música como não reproduzível", async ({ page }) => {
  const { pageErrors } = await openLibrary(page, {
    list_songs: { ok: songs },
    update_song: { ok: { ...songs[0], playable: "no" } },
  });
  await page.getByRole("button", { name: "Cantar Canção da Manhã, de Banda Aurora" }).click();
  await expect(page.getByRole("alert")).toContainText("Converta para MP4 com vídeo H.264 e áudio AAC.");
  await expect(page.getByRole("button", { name: "Começar" })).toHaveCount(0);
  await expect.poll(() => ipcCalls(page, "update_song")).toEqual([{ cmd: "update_song", args: { id: "a1", patch: { playable: "no" } } }]);
  expect(pageErrors).toEqual([]);
});
