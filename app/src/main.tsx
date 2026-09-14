import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import App from "./App";

// ── TEMPORARY DEBUG INSTRUMENTATION — remove once the white screen is fixed ──
// A render crash unmounts the tree and leaves a blank window with nothing to
// read. Paint whatever threw, on top, so it can be read off the screen.
function paint(label: string, err: unknown) {
  const e = err as { message?: string; stack?: string } | null;
  const box = document.createElement("pre");
  box.setAttribute("data-debug-overlay", "");
  box.style.cssText =
    "position:fixed;inset:0;z-index:2147483647;margin:0;padding:24px;" +
    "background:#1b1b1f;color:#ff9f9f;font:12px/1.5 ui-monospace,monospace;" +
    "white-space:pre-wrap;overflow:auto";
  box.textContent = `${label}\n\n${e?.message ?? String(err)}\n\n${e?.stack ?? ""}`;
  document.body.appendChild(box);
}
window.addEventListener("error", (ev) => paint("window error", ev.error ?? ev.message));
window.addEventListener("unhandledrejection", (ev) => paint("unhandled rejection", ev.reason));
// ── end temporary instrumentation ────────────────────────────────────────────

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
