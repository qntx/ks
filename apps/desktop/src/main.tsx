import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app";

import "./styles.css";

const root = document.querySelector("#app");
if (root === null) {
  throw new Error("missing #app root element");
}
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
