import type { WidgetRequest } from "../messages";
import type { AskGrant } from "@/stores/chat/widgetStore";
import { ASK_CAP, MAX_PROMPT, MAX_SCHEMA, MAX_TEXT, REFUSED } from "./limits";

/** What happens to a request: run it, hold it for the student's Allow, or
 *  refuse it with the reason the widget's promise rejects with. `capped`
 *  says the refusal is the cap, which the bar answers with Allow 20 more. */
export type Verdict = { run: true } | { hold: true } | { refuse: string; capped?: true };

/**
 * The host's rules for one `ask` or `send`, in order: one request at a time
 * (one held for Allow counts), sizes, a chat to send to, then the student's
 * answer and the cap. Nothing here reaches the frame; the iframe never
 * decides.
 */
export function gate(
  req: WidgetRequest,
  now: { grant: AskGrant | undefined; busy: boolean; canSend: boolean },
): Verdict {
  if (now.busy) return { refuse: REFUSED.busy };
  const size = sizeError(req);
  if (size) return { refuse: size };
  if (req.kind === "send" && !now.canSend) return { refuse: REFUSED.noChat };
  if (!now.grant) return { hold: true };
  if (!now.grant.allowed) return { refuse: REFUSED.denied };
  if (now.grant.used >= ASK_CAP) return { refuse: REFUSED.capped, capped: true };
  return { run: true };
}

function sizeError(req: WidgetRequest): string | null {
  if (req.kind === "send") {
    if (!req.text.trim()) return REFUSED.emptyText;
    return [...req.text].length > MAX_TEXT ? REFUSED.longText : null;
  }
  if (!req.prompt.trim()) return REFUSED.emptyPrompt;
  if ([...req.prompt].length > MAX_PROMPT) return REFUSED.longPrompt;
  return [...JSON.stringify(req.schema)].length > MAX_SCHEMA ? REFUSED.longSchema : null;
}

/** The grant after one request runs. */
export const spend = (grant: AskGrant): AskGrant => ({ ...grant, used: grant.used + 1 });
/** A fresh allowance: Allow, or Allow 20 more. */
export const ALLOWED: AskGrant = { allowed: true, used: 0 };
export const DENIED: AskGrant = { allowed: false, used: 0 };
