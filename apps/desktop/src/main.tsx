import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app/app";

import "./styles.css";

if (import.meta.env.DEV && new URLSearchParams(globalThis.location.search).has("mock")) {
  await import("./dev/mock-ipc");
}

const root = document.querySelector("#app");
if (root === null) {
  throw new Error("#app element missing");
}
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
