// T-013-04 (SPEC-013): UltraStar import against the stateful fake (e2e/fake-backend.ts). The parser itself is
// scoring_engine::import_ultrastar (Agent 2, tested in Rust + fixtures in tests/fixtures/references/ultrastar/).
import { expect, test } from "@playwright/test";
import type { RefNote } from "../src/lib/ipc";
import { DUET_NOTICE, installFakeBackend } from "./fake-backend";
import { fullSpec } from "./fixtures";
import { mockTauri, withIpc } from "./tauri-mock";

/** basic.txt of the fixtures (SPEC-013 criterion: BPM 300, GAP 1000). */
const basic: RefNote[] = [
  { startMs: 1000, durMs: 200, midi: 60, lyric: "Lá" },
  { startMs: 1200, durMs: 200, midi: 62, lyric: "ri" },
  { startMs: 1900, durMs: 200, midi: 64, lyric: "la" },
];
const ultrastar = {
  "basic.txt": { notes: basic },
  "duet.txt": { notes: basic.slice(0, 2), duet: true },
  "invalid-truncated-line.txt": { line: 4, message: "a nota precisa de início, duração, altura e sílaba" },
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
    rejected: { code: "invalid_reference", message: "Referência inválida: linha 4: a nota precisa de início, duração, altura e sílaba." },
  });
});
