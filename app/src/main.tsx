import React from "react";
import ReactDOM from "react-dom/client";
// Before ./App: the shim has to be in place ahead of the first `listen()`.
import "./lib/tauriEvents";
import "./index.css";
import App from "./App";

// A render crash unmounts the tree and leaves a blank window with nothing to
// read — there is no devtools console in a release webview and no Vite overlay
// for a runtime throw. So paint whatever threw, on top, to be read off the
// screen. Escape dismisses it, and a second error replaces the box rather than
// stacking another one: the point is to read the first thing that broke.
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

// "ResizeObserver loop completed with undelivered notifications" is the
// browser reporting its own frame budget, not a fault: a callback resized
// something the observer watches, so delivery continues next frame. It arrives
// as a window `error` event with a null `error` — indistinguishable from a
// crash to the listener below, and a window resize fires every observer in
// every open tab at once, so it blacked out a working app. Ignore it by name.
const BENIGN = /^ResizeObserver loop/;

window.addEventListener("error", (ev) => {
  if (BENIGN.test(ev.message ?? "")) return;
  paint("window error", ev.error ?? ev.message);
});
window.addEventListener("unhandledrejection", (ev) => paint("unhandled rejection", ev.reason));
window.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape") document.querySelector(`[${OVERLAY_ATTR}]`)?.remove();
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
