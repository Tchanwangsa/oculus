import type { CardState } from "./cardState";
import { object, type Row } from "./model";

/** A card's follow-up as Rust records it in the proposal state
 * (`harness/app/continuation.rs`): the agent's `then`, and `continuation`
 * once the card settled and claimed its one turn. */
export interface FollowUp {
  then: string | null;
  phase: "queued" | "started" | "skipped" | null;
  reason: string | null;
}

export function followUp(state: Row): FollowUp {
  const then = typeof state.then === "string" && state.then.trim() ? state.then.trim() : null;
  const claim = object(state.continuation);
  const phase = claim.phase === "queued" || claim.phase === "started" || claim.phase === "skipped" ? claim.phase : null;
  return { then, phase, reason: typeof claim.reason === "string" ? claim.reason : null };
}

/** A deciding button's words: with a follow-up, the click also starts the
 * agent's one billed turn, so the button says so ("Sync and continue"). */
export const continueLabel = (label: string, follow: FollowUp) => follow.then ? `${label} and continue` : label;

/** The line under a card with a follow-up, once it no longer waits on the
 * student: what will happen, what did, or why it never will. */
export function followUpNote(follow: FollowUp, frame: CardState): string | undefined {
  if (!follow.then) return undefined;
  if (follow.phase === "skipped") return follow.reason ?? "The agent did not continue.";
  if (follow.phase === "started") return "The agent continued below.";
  if (follow.phase === "queued") return "The agent continues after its current reply.";
  if (frame.phase === "applying" && !frame.locked) return "The agent continues when this finishes.";
  return undefined;
}
