/**
 * The messages between a widget frame and the reply that holds it
 * (docs/widgets.md#a-frame-talks-to-its-reply-only-through-postmessage).
 * Shared by the kit's runtime, the guard script `srcdoc.ts` inlines, and the
 * parent's filter, so the three never drift.
 */

/** `type` of every message a frame posts to its parent. */
export const WIDGET_MESSAGE = "oculus-widget";
/** `type` of every message the parent posts into a frame. */
export const HOST_MESSAGE = "oculus-host";

/** Where an error came from: the guard (`kit`, `csp`, `runtime`) or the
 *  runtime's error boundary (`render`). The parent adds `transpile` and
 *  `overflow` itself. */
export type ErrorPhase = "kit" | "csp" | "runtime" | "render";

export type WidgetMessage =
  | { type: typeof WIDGET_MESSAGE; kind: "height"; height: number }
  | { type: typeof WIDGET_MESSAGE; kind: "state"; state: Record<string, unknown> }
  | { type: typeof WIDGET_MESSAGE; kind: "error"; phase: ErrorPhase; message: string }
  /** Content wider than the frame: `body` clips it, so it is an error. */
  | { type: typeof WIDGET_MESSAGE; kind: "overflow"; width: number; frameWidth: number }
  /** `oculus/ask`'s `ask`: one model call answering JSON that matches
   *  `schema`. The parent answers with a `reply` carrying the same `id`. */
  | { type: typeof WIDGET_MESSAGE; kind: "ask"; id: string; prompt: string; schema: Record<string, unknown> }
  /** `oculus/ask`'s `send`: `text` as the student's next message in the chat. */
  | { type: typeof WIDGET_MESSAGE; kind: "send"; id: string; text: string };

/** A request the frame makes and the parent answers by `id`. */
export type WidgetRequest = Extract<WidgetMessage, { kind: "ask" | "send" }>;

export type HostMessage =
  /** The parent replacing the frame's state (the expanded copy closing). The
   *  runtime applies it without posting it back. */
  | { type: typeof HOST_MESSAGE; kind: "state"; state: Record<string, unknown> }
  /** The frame left the view (or the expanded copy covers it): a 3D scene
   *  swaps in a still and releases its WebGL context. */
  | { type: typeof HOST_MESSAGE; kind: "pause" }
  /** The frame is in view: a 3D scene draws again. */
  | { type: typeof HOST_MESSAGE; kind: "resume" }
  /** The answer to the frame's `ask` or `send` with this `id`. */
  | { type: typeof HOST_MESSAGE; kind: "reply"; id: string; ok: true; value: unknown }
  | { type: typeof HOST_MESSAGE; kind: "reply"; id: string; ok: false; error: string };

export type ReplyMessage = Extract<HostMessage, { kind: "reply" }>;

/** What a request's `id` may be: the frame makes them, the parent only
 *  echoes them. */
export const REQUEST_ID = /^[A-Za-z0-9_-]{1,64}$/;

/** The flag the runtime sets on `window` when its module has evaluated; the
 *  guard reads it on `load` to tell a kit that never loaded. */
export const KIT_FLAG = "__oculusKit";

/** The top-level binding the transpiled widget holds its component in, which
 *  the frame's appended mount line passes to the runtime. Must match
 *  `DEFAULT_BINDING` in `src-tauri/src/widgets/transpile.rs`. */
export const DEFAULT_BINDING = "__oculusWidget";
