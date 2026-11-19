// Usage (Node ≥ 22.18 runs .ts directly): pnpm --filter desktop exec node e2e/tools/gen-test-song.ts <pasta-de-saída>
// Writes mon-test-song.webm, mon-test-song-karaoke.webm and mon-test-song.mon-melody.json. Takes ~1 min (real time).
import { chromium } from "@playwright/test";
import { FILES, generateTestSong } from "./test-song.ts";

declare const process: { argv: string[]; exitCode?: number }; // no @types/node in this app

const outDir = process.argv[2];
if (!outDir) {
  console.error("Uso: node e2e/tools/gen-test-song.ts <pasta-de-saída>");
  process.exitCode = 2;
} else {
  const browser = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
  try {
    const page = await browser.newPage();
    console.log("Gerando (tempo real, ~1 min)…");
    const report = await generateTestSong(page, outDir);
    console.log(report);
    console.log(`Pronto em ${outDir}: ${Object.values(FILES).join(", ")}`);
  } finally {
    await browser.close();
  }
}
