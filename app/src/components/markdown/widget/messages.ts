import {
  HOST_MESSAGE,
  REQUEST_ID,
  WIDGET_MESSAGE,
  type ErrorPhase,
  type HostMessage,
  type ReplyMessage,
  type WidgetMessage,
  type WidgetRequest,
} from "../../../../widget-kit/protocol";

export type { WidgetMessage, WidgetRequest };

const PHASES: ReadonlySet<string> = new Set<ErrorPhase>(["kit", "csp", "runtime", "render"]);
const MAX_ERROR = 500;

const isRecord = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object" && !Array.isArray(v);
const finite = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);
const requestId = (v: unknown): v is string => typeof v === "string" && REQUEST_ID.test(v);

/**
 * A widget message from `frame`, or null. Every frame and embed on the page
 * posts to the same window, and a widget is untrusted code, so a message
 * counts only when its source is this frame's own window and its shape is
 * exactly one of the protocol's.
 */
export function readWidgetMessage(e: { source: unknown; data: unknown }, frame: Window | null | undefined): WidgetMessage | null {
  if (!frame || e.source !== frame) return null;
  const d = e.data;
  if (!isRecord(d) || d.type !== WIDGET_MESSAGE) return null;
  switch (d.kind) {
    case "height":
      return finite(d.height) && d.height >= 0 ? { type: WIDGET_MESSAGE, kind: "height", height: Math.ceil(d.height) } : null;
    case "state":
      return isRecord(d.state) ? { type: WIDGET_MESSAGE, kind: "state", state: d.state } : null;
    case "error":
      return typeof d.message === "string" && typeof d.phase === "string" && PHASES.has(d.phase)
        ? { type: WIDGET_MESSAGE, kind: "error", phase: d.phase as ErrorPhase, message: d.message.slice(0, MAX_ERROR) }
        : null;
    case "overflow":
      return finite(d.width) && finite(d.frameWidth)
        ? { type: WIDGET_MESSAGE, kind: "overflow", width: Math.ceil(d.width), frameWidth: Math.floor(d.frameWidth) }
        : null;
    // Sizes are the gate's to refuse (`ask/gate.ts`), with a reply; a shape
    // that is wrong gets none, since the kit never posts one.
    case "ask":
      return requestId(d.id) && typeof d.prompt === "string" && isRecord(d.schema)
        ? { type: WIDGET_MESSAGE, kind: "ask", id: d.id, prompt: d.prompt, schema: d.schema }
        : null;
    case "send":
      return requestId(d.id) && typeof d.text === "string"
        ? { type: WIDGET_MESSAGE, kind: "send", id: d.id, text: d.text }
        : null;
    default:
      return null;
  }
}

/** Hands `state` to a frame (the expanded copy closing); it isn't posted back. */
export function sendState(frame: Window | null | undefined, state: Record<string, unknown>) {
  const message: HostMessage = { type: HOST_MESSAGE, kind: "state", state };
  // A sandboxed frame's origin is opaque, so `*`; only that window receives it.
  frame?.postMessage(message, "*");
}

/** Answers the frame's request `id`: its value, or why it was refused. */
export function sendReply(frame: Window | null | undefined, id: string, result: { value: unknown } | { error: string }) {
  const message: ReplyMessage =
    "error" in result
      ? { type: HOST_MESSAGE, kind: "reply", id, ok: false, error: result.error }
      : { type: HOST_MESSAGE, kind: "reply", id, ok: true, value: result.value };
  frame?.postMessage(message, "*");
}

/** Tells a frame whether it is in view: a 3D scene draws only while live and
 *  releases its WebGL context on `pause`. A frame without one ignores it. */
export function sendVisibility(frame: Window | null | undefined, live: boolean) {
  const message: HostMessage = { type: HOST_MESSAGE, kind: live ? "resume" : "pause" };
  frame?.postMessage(message, "*");
}

/**
 * What to post a frame about its visibility, or null: nothing before its
 * first height report (a message sent before its runtime listens is lost),
 * then the live flag once, then only when it changes. `sent` is the flag last
 * posted to this document, null for none.
 */
export function visibilityToPost(sent: boolean | null, live: boolean, spoken: boolean): boolean | null {
  if (!spoken || sent === live) return null;
  return live;
}
