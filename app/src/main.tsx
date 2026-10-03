import React from "react";
import ReactDOM from "react-dom/client";
// Before ./App: the shim has to be in place ahead of the first `listen()`.
import "./lib/tauriEvents";
import "./index.css";
import App from "./App";
import { loadIconCatalogue } from "./components/subjects/SubjectIcon";
import { useSubjectIconStore } from "./stores/subjectIconStore";

// A release webview has no console, so paint uncaught errors over the window.
// Escape dismisses; a later error replaces the box rather than stacking.
const OVERLAY_ATTR = "data-debug-overlay";

function paint(label: string, err: unknown) {
  const e = err as { message?: string; stack?: string } | null;
  const box = document.createElement("pre");
  box.setAttribute(OVERLAY_ATTR, "");
  box.style.cssText =
    "position:fixed;inset:0;z-index:2147483647;margin:0;padding:24px;" +
    "background:#1b1b1f;color:#ff9f9f;font:12px/1.5 ui-monospace,monospace;" +
    "white-space:pre-wrap;overflow:auto";
  box.textContent =
    `${label}  —  press Esc to dismiss\n\n` +
    `${e?.message ?? String(err)}\n\n${e?.stack ?? ""}`;
  document.querySelector(`[${OVERLAY_ATTR}]`)?.remove();
  document.body.appendChild(box);
}

// Not faults: the ResizeObserver loop notice is the browser deferring delivery
// a frame, and the `previousSibling` one is pdf.js's unguarded global
// `selectionchange` handler. A bare identifier only, so our own
// `p.node.previousSibling` still surfaces.
const BENIGN = /^ResizeObserver loop|evaluating '\w+\.previousSibling'/;

window.addEventListener("error", (ev) => {
  if (BENIGN.test(ev.message ?? "")) return;
  paint("window error", ev.error ?? ev.message);
});
window.addEventListener("unhandledrejection", (ev) => paint("unhandled rejection", ev.reason));
window.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape") document.querySelector(`[${OVERLAY_ATTR}]`)?.remove();
});

function render() {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

// A stored custom glyph would first paint as a Book, so its catalogue loads
// before the first render. The store hydrates from localStorage synchronously;
// without a custom icon the catalogue stays off the startup path.
const customIcon = Object.values(useSubjectIconStore.getState().prefs).some((p) => p.icon);
if (customIcon) void loadIconCatalogue().catch(() => {}).finally(render);
else render();
