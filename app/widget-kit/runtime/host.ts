import { HOST_MESSAGE, REQUEST_ID, type HostMessage } from "../protocol";

/** A message from the parent, exactly one of the protocol's shapes. */
export function isHostMessage(data: unknown): data is HostMessage {
  const d = data as Record<string, unknown> | null;
  if (!d || typeof d !== "object" || d.type !== HOST_MESSAGE) return false;
  if (d.kind === "pause" || d.kind === "resume") return true;
  if (d.kind === "reply") {
    if (typeof d.id !== "string" || !REQUEST_ID.test(d.id)) return false;
    return (d.ok === true && "value" in d) || (d.ok === false && typeof d.error === "string");
  }
  return d.kind === "state" && !!d.state && typeof d.state === "object" && !Array.isArray(d.state);
}

/** `e` as a host message when it comes from `from` (the runtime passes
 *  `parent`: another frame can't reach this window, but be exact). */
export function hostMessage(e: { source: unknown; data: unknown }, from: unknown): HostMessage | null {
  return e.source === from && isHostMessage(e.data) ? e.data : null;
}
