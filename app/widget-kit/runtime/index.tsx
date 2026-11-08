/**
 * The widget runtime: the module the `srcdoc`'s script imports
 * (docs/widgets.md). Evaluating it marks the kit loaded for the guard
 * script. `mount` boots a committed widget: it seeds the state, renders the
 * widget under an error boundary into `#root` and starts the height
 * reports. `stream` boots a frame for a reply still streaming, which draws
 * the programs the parent posts (`stream.tsx`). Errors outside React (event
 * handlers, timers, rejections) are the guard's. The parent's `pause`/`resume`
 * go to the visibility store `oculus/3d` reads, and its `reply`s to the
 * requests `oculus/ask` is waiting on.
 */
import type { ComponentType } from "react";
import { createRoot, type Root } from "react-dom/client";
import { KIT_FLAG } from "../protocol";
import { onPublish, replaceState, type State } from "../state/store";
import { settle } from "../ask/pending";
import { Boundary } from "./Boundary";
import { hostMessage } from "./host";
import { measure } from "./measure";
import { isStreaming } from "./mode";
import { describe, post } from "./post";
import { receive, startStream } from "./stream";
import { setVisibility } from "./visibility";

export { commit, update } from "./stream";
export { previewScope } from "./preview";
export { pendingPart } from "./Pending";

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

/** What both boots share: the seed, state posts, the parent's messages. */
function listen() {
  replaceState(seed());
  // A preview's state is the program's own, not the student's.
  onPublish((state) => !isStreaming() && post({ kind: "state", state }));
  // Registered before the first height report, which tells the parent it may post.
  addEventListener("message", (e) => {
    const m = hostMessage(e, parent);
    if (!m) return;
    if (m.kind === "state") replaceState(m.state);
    else if (m.kind === "reply") settle(m);
    else if (m.kind === "module") receive(m);
    else setVisibility(m.kind === "resume" ? "live" : "paused");
  });
  const root = document.getElementById("root");
  if (!root) post({ kind: "error", phase: "kit", message: "The frame has no #root" });
  return root;
}

export function mount(Widget: unknown) {
  const root = listen();
  if (!root) return;
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

export function stream() {
  const root = listen();
  if (!root) return;
  const react: Root = createRoot(root);
  startStream(react);
  measure(root);
}
