import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";
import type { PluginOption } from "vite-plus";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [...react()] as PluginOption[],
  // Tauri prints its own banner; a cleared screen hides the Rust errors.
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host === undefined ? undefined : { protocol: "ws", host, port: 1421 },
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: {
    environment: "happy-dom",
    globals: false,
  },
  build: {
    // Tauri 2 webview floors: WKWebView on macOS, Chromium on Windows.
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: process.env.TAURI_ENV_DEBUG === undefined,
    sourcemap: process.env.TAURI_ENV_DEBUG !== undefined,
  },
});
