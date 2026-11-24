/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
    // Deterministic under load (other agents compiling Rust): vitest's worker start timeout is a fixed 60 s,
    // so cap concurrent jsdom workers instead of one per core. A failed worker still exits non-zero.
    pool: "threads",
    maxWorkers: 1,
    testTimeout: 15_000,
  },

  // Tauri: keep Rust errors on screen, fixed port (tauri.conf.json devUrl), don't watch src-tauri.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
