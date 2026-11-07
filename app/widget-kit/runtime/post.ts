import { WIDGET_MESSAGE, type WidgetMessage } from "../protocol";

type Body = WidgetMessage extends infer M ? (M extends { type: string } ? Omit<M, "type"> : never) : never;

/** Posts to the parent. `*` because the parent's origin is the app's, which
 *  a sandboxed frame can't name; the parent checks the source instead. */
export function post(message: Body) {
  try {
    parent.postMessage({ type: WIDGET_MESSAGE, ...message }, "*");
  } catch {
    // A value that won't clone: nothing to report it to.
  }
}

export function describe(error: unknown): string {
  if (error instanceof Error) return error.name && error.name !== "Error" ? `${error.name}: ${error.message}` : error.message;
  return String(error);
}
