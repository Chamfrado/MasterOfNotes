// E2E against the Vite dev server (no Tauri). IPC is mocked only inside e2e/ (Law 03).
// Run: pnpm --filter desktop exec playwright test
import { defineConfig, devices } from "@playwright/test";

declare const process: { env: Record<string, string | undefined> }; // no @types/node in this app

// Default 1440 (Agent 4); set E2E_PORT to run several worktrees at once. `tauri dev` owns 1420.
const PORT = Number(process.env.E2E_PORT ?? 1440);
const CI = !!process.env.CI;

export default defineConfig({
  testDir: "e2e",
  testMatch: "**/*.e2e.ts",
  forbidOnly: CI,
  retries: CI ? 1 : 0,
  reporter: CI ? "github" : "list",
  use: { baseURL: `http://localhost:${PORT}`, trace: "retain-on-failure" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] }, testIgnore: /test-song\.e2e\.ts/ },
    // Real-time recording drops video frames under CPU load (measured: 16 instead of ~27 fps, stale frame
    // after seek), so it runs alone, after everything else. Run it by itself with `--no-deps`.
    { name: "test-song", use: { ...devices["Desktop Chrome"] }, testMatch: /test-song\.e2e\.ts/, dependencies: ["chromium"] },
  ],
  webServer: {
    command: `pnpm exec vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    // Never reuse: a server already on the port may be another worktree's code. Fail loudly instead.
    reuseExistingServer: false,
  },
});
