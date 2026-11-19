// T-008-04: Party flow end to end (SPEC-008) against the stateful test-only fake (e2e/fake-backend.ts).
import { expect, test, type Page } from "@playwright/test";
import { installFakeBackend } from "./fake-backend";
import { fullSpec, settings, song, songs } from "./fixtures";
import { mockTauri, useSyntheticVideo } from "./tauri-mock";

const partySongs = [
  ...songs, // a1, b2: no reference; c3: validated; d4: unavailable
  song("e5", "Estrela Cadente", "Banda Aurora", { referenceStatus: "validated" }),
  song("f6", "Rua da Chuva", null, { referenceStatus: "validated" }),
];
// A passed leak check for this device/volume, so validated songs start as competitive ("Começar").
const competitiveSettings = settings("C:/Karaoke", { leakCheck: { deviceId: null, videoVolume: 0.8, passed: true, atMs: 0 } });

const nav = (page: Page, label: string) =>
  page.getByRole("navigation", { name: "Principal" }).getByRole("button", { name: label }).click();

/** Sings the current round of the open session; the fake scores it with the next queued score. */
async function singCurrentRound(page: Page, who: string, title: string) {
  await expect(page.getByRole("heading", { level: 2, name: `${who} canta ${title}` })).toBeVisible();
  await page.getByRole("button", { name: "Cantar", exact: true }).click();
  await page.getByRole("button", { name: "Começar", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Resultado" })).toBeVisible({ timeout: 15_000 }); // 3 s countdown + video
  await page.getByRole("button", { name: "Voltar para a partida" }).click();
}

const standingRow = (page: Page, name: string) =>
  page.getByRole("table", { name: "Ranking da festa" }).getByRole("row").filter({ has: page.getByRole("rowheader", { name: new RegExp(`^${name}`) }) });

test("festa: cantores, só músicas validadas, rodadas, ranking por média com total, retomada após fechar e vencedor", async ({ page }) => {
  test.setTimeout(120_000);
  await installFakeBackend(page, [80, 90, 70]);
  const { pageErrors } = await mockTauri(page, fullSpec({ get_settings: { ok: competitiveSettings }, list_songs: { ok: partySongs } }));
  await page.goto("/");
  await useSyntheticVideo(page);

  // Participants: two singers; a duplicate name is refused by the backend with its message.
  await nav(page, "Jogar");
  await page.getByRole("button", { name: "Participantes" }).click();
  const newSinger = page.getByLabel("Novo cantor");
  for (const name of ["Ana", "Bruno", "ana"]) {
    await newSinger.fill(name);
    await newSinger.press("Enter");
  }
  await expect(page.getByText('Já existe um cantor chamado "ana".')).toBeVisible();
  await expect(page.getByRole("list", { name: "Cantores" }).getByRole("listitem")).toHaveText(["Ana", "Bruno"]);
  await page.keyboard.press("Escape");

  // New party: songs without a validated reference are not offered, and the screen says why.
  await page.getByRole("button", { name: /^Festa/ }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Nova partida: Festa" })).toBeVisible();
  await expect(page.getByText(/2 músicas não aparecem porque ainda não têm referência validada/)).toBeVisible();
  await expect(page.getByRole("checkbox")).toHaveCount(3);
  await expect(page.getByText("Canção da Manhã")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Criar festa" })).toBeDisabled();
  await expect(page.getByRole("list", { name: "O que falta" })).toContainText("Dê um nome à festa.");

  await page.getByLabel("Nome da festa").fill("Festa de Teste");
  await page.getByLabel("Na ordem em que escolhi cantores e músicas").check();
  await page.getByRole("button", { name: /Ana$/ }).click();
  await page.getByRole("button", { name: /Bruno$/ }).click();
  for (const title of ["Pôr do Sol em Óculos", "Estrela Cadente", "Rua da Chuva"]) await page.getByRole("checkbox", { name: new RegExp(title) }).check();
  await page.getByRole("button", { name: "Criar festa" }).click();

  // Round 1 (Ana 80). Bruno has not sung: listed but ineligible (minRounds default = 1).
  await expect(page.getByRole("heading", { level: 1, name: "Festa de Teste" })).toBeVisible();
  await expect(page.getByText("Rodada 1 de 3")).toBeVisible();
  await singCurrentRound(page, "Ana", "Pôr do Sol em Óculos");
  await expect(page.getByRole("heading", { name: "Ranking parcial" })).toBeVisible();
  await expect(standingRow(page, "Ana").getByRole("cell")).toHaveText(["1", "80", "80", "1"]);
  await expect(standingRow(page, "Bruno")).toContainText("Inelegível: falta 1 rodada");

  // "Close the app" and resume from Jogar → Continuar de onde parou.
  await page.reload();
  await useSyntheticVideo(page);
  await nav(page, "Jogar");
  const resume = page.getByRole("region", { name: "Continuar de onde parou" });
  await expect(resume).toContainText("Festa de Teste");
  await expect(resume).toContainText("faltam 2 rodadas");
  await resume.getByRole("button", { name: "Retomar" }).click();

  // Rounds 2 (Bruno 90) and 3 (Ana 70): Bruno leads by AVERAGE (90) despite the lower TOTAL (90 < 150).
  await singCurrentRound(page, "Bruno", "Estrela Cadente");
  await singCurrentRound(page, "Ana", "Rua da Chuva");
  const rows = page.getByRole("table", { name: "Ranking da festa" }).getByRole("row");
  await expect(rows.nth(1).getByRole("rowheader")).toHaveText("Bruno");
  await expect(standingRow(page, "Bruno").getByRole("cell")).toHaveText(["1", "90", "90", "1"]);
  await expect(standingRow(page, "Ana").getByRole("cell")).toHaveText(["2", "75", "150", "2"]);

  // No rounds left: Encerrar completes at once and the winner is announced.
  await page.getByRole("button", { name: "Encerrar" }).click();
  await expect(page.getByRole("heading", { name: "Classificação final" })).toBeVisible();
  await expect(page.getByText("Vencedor da festa")).toBeVisible();
  await expect(page.getByText("Vencedor da festa").locator("..")).toContainText("Bruno");
  expect(pageErrors).toEqual([]);
});
