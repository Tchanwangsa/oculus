/**
 * A frame booted to stream (docs/widgets.md#a-widget-draws-while-it-streams):
 * the parent posts each program as a `module` message, the runtime runs it
 * as a module carrying the document's nonce, and the module hands its
 * component to `update`, which draws it in place of the last. The root, the
 * frame and `oculus/state` stay, so state kept by name carries over.
 *
 * A preview that throws — at the top level, in render or in an effect —
 * leaves the last good preview drawn and reports nothing. The `final`
 * program's first statement calls `commit`: previews' timers are retired,
 * the gates open, and from then on it fails like any committed widget. The
 * gate opens there, not when the message arrives, so a preview still
 * evaluating when the final arrives stays quiet.
 */
import { useEffect, type ComponentType } from "react";
import type { Root } from "react-dom/client";
import type { ModuleMessage } from "../protocol";
import { Boundary } from "./Boundary";
import { setStreaming } from "./mode";
import { describe, post } from "./post";
import { retire } from "./preview";

/** Good previews kept to fall back on, newest last. */
const KEPT = 3;

let root: Root | null = null;
let nonce = "";
/** The newest program asked for; a module older than it is stale. */
let latest = 0;
let finalId = 0;
const scripts = new Map<number, HTMLScriptElement>();
const failed = new Set<number>();
/** The preview on screen, and the newest ones that drew without throwing. */
let shown = 0;
let good: { id: number; Widget: ComponentType }[] = [];

/** Starts streaming into `target`: nothing drawn until the first program. */
export function startStream(target: Root) {
  root = target;
  nonce = document.querySelector<HTMLScriptElement>("script[nonce]")?.nonce ?? "";
  setStreaming(true);
}

/** Runs the parent's next program. */
export function receive(m: ModuleMessage) {
  if (!root || finalId || m.id <= latest) return;
  latest = m.id;
  if (m.final) finalId = m.id;
  const script = document.createElement("script");
  script.type = "module";
  script.nonce = nonce;
  script.textContent = m.code;
  scripts.set(m.id, script);
  document.head.appendChild(script);
}

/** The final program's first statement: the stream is over. */
export function commit(id: number) {
  if (id !== finalId) return;
  retire(id);
  setStreaming(false);
}

/** Marks a preview good once it has committed and its effects ran clean. */
function Settled({ id, Widget }: { id: number; Widget: ComponentType }) {
  useEffect(() => {
    const timer = setTimeout(() => {
      if (failed.has(id) || good.some((g) => g.id >= id)) return;
      good = [...good, { id, Widget }].slice(-KEPT);
    }, 0);
    return () => clearTimeout(timer);
  }, [id, Widget]);
  return null;
}

function show(id: number, Widget: ComponentType) {
  shown = id;
  if (id === finalId) {
    root?.render(
      <Boundary key={id}>
        <Widget />
      </Boundary>,
    );
    return;
  }
  root?.render(
    <Boundary key={id} onError={() => fail(id)}>
      <Widget />
      <Settled id={id} Widget={Widget} />
    </Boundary>,
  );
}

/** A preview threw: the newest good one other than it goes back on screen. */
function fail(id: number) {
  failed.add(id);
  good = good.filter((g) => g.id !== id);
  const back = good[good.length - 1];
  if (id === shown && back) show(back.id, back.Widget);
}

/** Called by each program's module with its component. */
export function update(Widget: unknown, id: number) {
  for (const [key, script] of scripts) {
    if (key > id) continue;
    script.remove();
    scripts.delete(key);
  }
  // A newer program is already on its way; this one would only flash.
  if (!root || id !== latest) return;
  const valid = typeof Widget === "function" || (typeof Widget === "object" && Widget !== null);
  if (id !== finalId) {
    retire(id);
    if (valid) show(id, Widget as ComponentType);
    return;
  }
  if (!valid) return post({ kind: "error", phase: "render", message: "The widget's default export is not a component" });
  try {
    show(id, Widget as ComponentType);
  } catch (error) {
    post({ kind: "error", phase: "render", message: describe(error) });
  }
}
