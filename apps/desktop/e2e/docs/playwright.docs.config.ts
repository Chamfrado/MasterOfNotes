// SPEC-011 T-011-02: regenerates every image, GIF and video of the user guide (docs/user/media/).
// Run: pnpm --filter desktop exec playwright test -c e2e/docs/playwright.docs.config.ts
// Not a test suite: it is kept out of the normal E2E run (testMatch there is *.e2e.ts).
import { defineConfig, devices } from "@playwright/test";

declare const process: { env: Record<string, string | undefined> }; // no @types/node in this app

const PORT = Number(process.env.DOCS_PORT ?? 1450); // Agent 5's Vite port; never reuse another server

export default defineConfig({
  testDir: ".",
  testMatch: "*.docs.ts",
  outputDir: "test-results/run",
  workers: 1, // captures are timing-sensitive (GIF frames, video): one page at a time
  timeout: 180_000,
  reporter: "list",
  use: {
    ...devices["Desktop Chrome"],
    baseURL: `http://localhost:${PORT}`,
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 1,
    locale: "pt-BR",
    timezoneId: "America/Sao_Paulo",
    launchOptions: { args: ["--autoplay-policy=no-user-gesture-required"] },
  },
  webServer: {
    command: `pnpm exec vite --port ${PORT} --strictPort`,
    cwd: "../..",
    url: `http://localhost:${PORT}`,
    reuseExistingServer: false,
  },
});
