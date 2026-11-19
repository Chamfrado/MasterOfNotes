// E2E against the Vite dev server (no Tauri). IPC is mocked only inside e2e/ (Law 03).
// Run: pnpm --filter desktop exec playwright test
import { defineConfig, devices } from "@playwright/test";

declare const process: { env: Record<string, string | undefined> }; // no @types/node in this app

const PORT = 1440; // Agent 4's port; `tauri dev` owns 1420.
const CI = !!process.env.CI;

export default defineConfig({
  testDir: "e2e",
  testMatch: "**/*.e2e.ts",
  forbidOnly: CI,
  retries: CI ? 1 : 0,
  reporter: CI ? "github" : "list",
  use: { baseURL: `http://localhost:${PORT}`, trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: `pnpm exec vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !CI,
  },
});
