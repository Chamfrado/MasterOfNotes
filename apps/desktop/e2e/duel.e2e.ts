// T-008-05: Duel verdict comes only from session.duel (Rust: game::duel_outcome with the CompatKeys).
// The stateful fake mirrors it: tenths, draw <=> margin 0, or "incompatible" when the keys differ.
import { expect, test } from "@playwright/test";
import { installFakeBackend } from "./fake-backend";
import { competitiveSettings, fullSpec, nav, partySongs, singCurrentRound } from "./fixtures";
import { mockTauri, useSyntheticVideo } from "./tauri-mock";

const cases = [
  { name: "vencedor por margem", scores: [80, 92.5], incompatible: false },
  { name: "empate após arredondar a 0,1", scores: [77.04, 76.96], incompatible: false },
  { name: "chaves incompatíveis: sem vencedor mesmo com notas diferentes", scores: [90, 60], incompatible: true },
];

for (const c of cases) {
  test(`duelo: ${c.name}; "Aguardando" antes da 2ª rodada`, async ({ page }) => {
    test.setTimeout(90_000);
    await installFakeBackend(page, c.scores, {
      participants: [
        { id: 1, name: "Ana" },
        { id: 2, name: "Bruno" },
      ],
      incompatibleDuel: c.incompatible,
    });
    const { pageErrors } = await mockTauri(page, fullSpec({ get_settings: { ok: competitiveSettings }, list_songs: { ok: partySongs } }));
    await page.goto("/");
    await useSyntheticVideo(page);

    await nav(page, "Jogar");
    await page.getByRole("button", { name: /^Duelo/ }).click();
    await expect(page.getByRole("heading", { level: 1, name: "Nova partida: Duelo" })).toBeVisible();
    await page.getByRole("button", { name: "Ana", exact: true }).click();
    await page.getByRole("button", { name: "Bruno", exact: true }).click();
    await page.getByRole("radio", { name: /Estrela Cadente/ }).check();
    await page.getByRole("button", { name: "Criar duelo" }).click();

    const board = page.getByRole("region", { name: "Confronto" });
    await singCurrentRound(page, "Ana", "Estrela Cadente");
    await expect(board).toContainText("Aguardando: o duelo termina quando os dois cantarem.");
    await expect(board).not.toContainText("venceu");

    await singCurrentRound(page, "Bruno", "Estrela Cadente");
    await expect(board).not.toContainText("Aguardando");
    if (c.incompatible) {
      const verdict = board.getByRole("status");
      await expect(verdict).toContainText("Duelo sem vencedor");
      await expect(verdict).toContainText("as performances não são comparáveis (referência, revisão, algoritmo ou dificuldade diferentes)");
      await expect(verdict).toContainText("As duas apresentações não podem ser comparadas");
      await expect(board).not.toContainText("venceu");
      await expect(board).not.toContainText("Empate");
    } else if (c.scores[0] === 80) {
      await expect(board).toContainText("Bruno venceu o duelo por 12,5 pontos!");
    } else {
      await expect(board).toContainText("Empate!");
      await expect(board).not.toContainText("venceu");
    }
    expect(pageErrors).toEqual([]);
  });
}
