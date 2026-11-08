import { STILL_WRITING } from "../../../../../widget-kit/protocol";
import type { WidgetRequest } from "../messages";
import type { AskReply } from "../ask/useWidgetAsk";

export { STILL_WRITING };

/**
 * The parent's half of the streaming gate: while the reply streams, a
 * frame's requests are refused and its state is not saved; once it commits,
 * both go where they always do. The frame refuses first
 * (`widget-kit/ask/pending.ts`), but it is untrusted, so the parent does too.
 */
export function streamGate(
  streaming: boolean,
  onRequest: ((req: WidgetRequest, reply: AskReply) => void) | undefined,
  onState: (state: Record<string, unknown>) => void,
): {
  onRequest: ((req: WidgetRequest, reply: AskReply) => void) | undefined;
  onState: (state: Record<string, unknown>) => void;
} {
  if (!streaming) return { onRequest, onState };
  return { onRequest: (_req, reply) => reply({ error: STILL_WRITING }), onState: () => {} };
}
