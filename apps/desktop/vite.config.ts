import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";
import type { PluginOption } from "vite-plus";

const host = process.env["TAURI_DEV_HOST"];

export default defineConfig({
  plugins: [...react(), tailwindcss()] as PluginOption[],
  // Tauri prints its own banner; a cleared screen hides the Rust errors.
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    ...(host === undefined ? {} : { hmr: { protocol: "ws" as const, host, port: 1421 } }),
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: {
    environment: "happy-dom",
    globals: false,
    setupFiles: ["./src/test/setup.ts"],
  },
  build: {
    // Tauri 2 webview floors: WKWebView 16 (macOS 13 floor in tauri.conf), Chromium 105 on Windows.
    target: process.env["TAURI_ENV_PLATFORM"] === "windows" ? "chrome105" : "safari16",
    minify: process.env["TAURI_ENV_DEBUG"] === undefined,
    sourcemap: process.env["TAURI_ENV_DEBUG"] !== undefined,
  },
});
