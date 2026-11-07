/**
 * The widget runtime: the module the `srcdoc`'s widget script imports to
 * mount its default export (docs/widgets.md). Evaluating it marks the kit
 * loaded for the guard script; `mount` seeds the state, renders the widget
 * under an error boundary into `#root` and starts the height reports.
 * Errors outside React (event handlers, timers, rejections) are the guard's.
 * The parent's `pause`/`resume` go to the visibility store `oculus/3d` reads,
 * and its `reply`s to the requests `oculus/ask` is waiting on.
 */
import type { ComponentType } from "react";
import { createRoot } from "react-dom/client";
import { KIT_FLAG } from "../protocol";
import { onPublish, replaceState, type State } from "../state/store";
import { settle } from "../ask/pending";
import { Boundary } from "./Boundary";
import { hostMessage } from "./host";
import { measure } from "./measure";
import { describe, post } from "./post";
import { setVisibility } from "./visibility";

(window as unknown as Record<string, unknown>)[KIT_FLAG] = true;

function seed(): State {
  const text = document.getElementById("oculus-state")?.textContent;
  if (!text) return {};
  try {
    const value: unknown = JSON.parse(text);
    return value && typeof value === "object" && !Array.isArray(value) ? (value as State) : {};
  } catch {
    return {};
  }
}

export function mount(Widget: unknown) {
  replaceState(seed());
  onPublish((state) => post({ kind: "state", state }));
  // Registered before the first height report, which tells the parent it may post.
  addEventListener("message", (e) => {
    const m = hostMessage(e, parent);
    if (!m) return;
    if (m.kind === "state") replaceState(m.state);
    else if (m.kind === "reply") settle(m);
    else setVisibility(m.kind === "resume" ? "live" : "paused");
  });
  const root = document.getElementById("root");
  if (!root) return post({ kind: "error", phase: "kit", message: "The frame has no #root" });
  if (typeof Widget !== "function" && (typeof Widget !== "object" || Widget === null)) {
    return post({ kind: "error", phase: "render", message: "The widget's default export is not a component" });
  }
  try {
    const App = Widget as ComponentType;
    createRoot(root).render(
      <Boundary>
        <App />
      </Boundary>,
    );
  } catch (error) {
    post({ kind: "error", phase: "render", message: describe(error) });
  }
  measure(root);
}
