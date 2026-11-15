import type { Capabilities } from "./capabilities";
import type { Provider } from "./providers";
import type { HarnessRun, RunHandoff } from "./types";

/** Whether a started chat on `current` cannot move to `target`: another
 *  provider's bridge has to take a handoff (`providerHandoff`). The same
 *  provider is never locked, and neither is anything before the chat has
 *  started (`current` null). Rust refuses the same switch with a reason. */
export function switchLocked(current: Provider | null, target: Provider, caps: (provider: Provider) => Capabilities): boolean {
  return current != null && current !== target && !caps(target).providerHandoff;
}

/** What a run was sent with that the run before it was not: the sides of a
 *  `{from} → {to}` divider, each only where it changed. A model the CLI
 *  resolved differently is not a change of the student's. Null when the two
 *  runs match, and when either is from before selections were recorded. */
export interface RunSwitch {
  provider?: [Provider, Provider];
  model?: [string | null, string | null];
  effort?: [string | null, string | null];
}

export function runSwitch(before: HarnessRun | undefined, run: HarnessRun): RunSwitch | null {
  if (!before?.provider || !run.provider) return null;
  const change: RunSwitch = {};
  if (before.provider !== run.provider) change.provider = [before.provider, run.provider];
  if ((before.model ?? null) !== (run.model ?? null)) change.model = [before.model ?? null, run.model ?? null];
  if ((before.effort ?? null) !== (run.effort ?? null)) change.effort = [before.effort ?? null, run.effort ?? null];
  return Object.keys(change).length ? change : null;
}

/** A handoff divider's words and tone: pending while its prompt is on the
 *  way, danger when it was never delivered. */
export function handoffLabel(handoff: RunHandoff): { label: string; tone: "neutral" | "danger" } {
  if (handoff.status === "pending") return { label: "Context handoff pending", tone: "neutral" };
  return { label: "Context handoff", tone: handoff.status === "failed" ? "danger" : "neutral" };
}
