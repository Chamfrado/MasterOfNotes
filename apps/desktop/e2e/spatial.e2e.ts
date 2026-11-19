// A11Y-05: arrow-key spatial navigation (src/spatial.ts) for TV remotes. Assertions are geometric, so they
// hold for any grid width.
import { expect, test, type Page } from "@playwright/test";
import { fullSpec } from "./fixtures";
import { mockTauri, useSyntheticVideo, type MockSpec } from "./tauri-mock";

test.use({ viewport: { width: 1366, height: 768 } });

const focused = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    const r = el.getBoundingClientRect();
    return {
      label: (el.getAttribute("aria-label") ?? el.textContent ?? "").trim(),
      inMain: !!el.closest("main"),
      inDialog: !!el.closest("dialog[open]"),
      top: r.top,
      bottom: r.bottom,
      left: r.left,
      right: r.right,
      cx: r.left + r.width / 2,
    };
  });

async function openLibrary(page: Page, spec: MockSpec = {}) {
  const { pageErrors } = await mockTauri(page, fullSpec(spec));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Principal" }).getByRole("button", { name: "Biblioteca" }).click();
  await expect(page.getByText("3 músicas", { exact: true })).toBeVisible();
  return pageErrors;
}

test("setas: na grade a direita leva para a direita na mesma linha, e a esquerda volta", async ({ page }) => {
  const pageErrors = await openLibrary(page);
  const start = page.getByRole("button", { name: "Cantar Canção da Manhã Banda Aurora" });
  await start.focus();
  const from = await focused(page);
  await page.keyboard.press("ArrowRight");
  const to = await focused(page);
  expect(to.label).not.toBe(from.label);
  expect(to.cx).toBeGreaterThan(from.cx);
  expect(to.top < from.bottom && from.top < to.bottom, `mesma linha: ${JSON.stringify({ from, to })}`).toBe(true);
  await page.keyboard.press("ArrowLeft");
  await expect(start).toBeFocused();
  expect(pageErrors).toEqual([]);
});

test("setas: da NavBar, seta para baixo entra no conteúdo da tela", async ({ page }) => {
  const pageErrors = await openLibrary(page);
  await page.getByRole("navigation", { name: "Principal" }).getByRole("button", { name: "Biblioteca" }).focus();
  await page.keyboard.press("ArrowDown");
  expect((await focused(page)).inMain).toBe(true);
  expect(pageErrors).toEqual([]);
});

test("setas: campo de busca fica com as setas; Enter ativa o botão focado", async ({ page }) => {
  const pageErrors = await openLibrary(page);
  const search = page.getByRole("searchbox", { name: "Buscar músicas" });
  await search.fill("canção");
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("ArrowDown");
  await expect(search).toBeFocused();
  await page.getByRole("button", { name: "Cantar Canção da Manhã Banda Aurora" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 1, name: "Tocando Canção da Manhã" })).toBeAttached();
  expect(pageErrors).toEqual([]);
});

test("setas: com diálogo aberto, o foco não sai dele", async ({ page }) => {
  const pageErrors = await openLibrary(page, {
    get_reference: { ok: { id: 3, songId: "c3", source: "json", status: "validated", revision: 1, offsetMs: 0, notes: [{ startMs: 0, durMs: 500, midi: 60, lyric: null }] } },
    list_midi_tracks: { ok: [{ index: 0, name: "Piano", noteCount: 120 }, { index: 1, name: "Voz", noteCount: 85 }] },
    "plugin:dialog|open": { ok: "C:/Karaoke/c3.mid" },
  });
  await page.getByRole("button", { name: "Referência de Pôr do Sol em Óculos" }).click();
  await page.getByRole("button", { name: "Importar outra" }).click();
  await expect(page.getByRole("dialog", { name: "Qual trilha é a voz?" })).toBeVisible();
  for (const key of ["ArrowDown", "ArrowDown", "ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"]) {
    await page.keyboard.press(key);
    expect((await focused(page)).inDialog, `após ${key}`).toBe(true);
  }
  expect(pageErrors).toEqual([]);
});

test("setas: no Player, ↑ aumenta o volume em vez de mover o foco", async ({ page }) => {
  const pageErrors = await openLibrary(page);
  await useSyntheticVideo(page, 6000);
  await page.getByRole("button", { name: "Cantar Canção da Manhã Banda Aurora" }).click();
  await page.getByRole("button", { name: "Começar" }).click();
  await page.getByRole("button", { name: "Pausar" }).click({ timeout: 10_000 });
  const resume = page.getByRole("button", { name: "Continuar" });
  await expect(resume).toBeFocused();
  const volume = page.getByRole("slider", { name: "Volume" });
  await expect(volume).toHaveAttribute("aria-valuetext", "80%");
  await page.keyboard.press("ArrowUp");
  await expect(volume).toHaveAttribute("aria-valuetext", "85%");
  await expect(resume).toBeFocused();
  expect(pageErrors).toEqual([]);
});
