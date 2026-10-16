import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/inter/opsz.css";
import "@fontsource-variable/geist-mono";
import { App } from "./app";
import "./app/styles/index.css";

const root = document.getElementById("root");
if (!root) throw new Error("the page has no #root element to mount into");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
