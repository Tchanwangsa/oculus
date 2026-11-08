/**
 * Whether this frame shows a streaming preview rather than a committed
 * widget (docs/widgets.md#a-widget-draws-while-it-streams). While it does,
 * `oculus/ask` refuses, state isn't posted and the guard keeps throws to
 * itself. Shared by the runtime and `ask.js` through one chunk.
 */
import { QUIET_FLAG } from "../protocol";

let streaming = false;

export const isStreaming = () => streaming;

export function setStreaming(next: boolean) {
  streaming = next;
  (window as unknown as Record<string, unknown>)[QUIET_FLAG] = next;
}
