import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app/app";
import { QuickApp } from "./features/quick";

import "./styles.css";

if (import.meta.env.DEV && new URLSearchParams(globalThis.location.search).has("mock")) {
  await import("./dev/mock-ipc");
}

/**
 * The `quick` window renders the minimal quick-access root; every other window is the main app. The
 * `?window=quick` query param wins so the quick root can be previewed in a browser (mock/dev).
 */
async function windowLabel(): Promise<string> {
  const forced = new URLSearchParams(globalThis.location.search).get("window");
  if (forced !== null && forced !== "") {
    return forced;
  }
  try {
    const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    return getCurrentWebviewWindow().label;
  } catch {
    return "main";
  }
}

const root = document.querySelector("#app");
if (root === null) {
  throw new Error("#app element missing");
}
const label = await windowLabel();
createRoot(root).render(<StrictMode>{label === "quick" ? <QuickApp /> : <App />}</StrictMode>);
