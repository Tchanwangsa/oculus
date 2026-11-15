import type { Capabilities } from "./capabilities";
import { reasoningLabel } from "./models";
import type { Provider } from "./providers";
import { providerLabel } from "./registry";
import { runModel } from "./runs";
import type { HarnessRun, HarnessThread, RunHandoff } from "./types";

/** Whether a started chat on `current` cannot move to `target`: another
 *  provider's bridge has to take a handoff (`providerHandoff`). The same
 *  provider is never locked, and neither is anything before the chat has
 *  started (`current` null). Rust refuses the same switch with a reason. */
export function switchLocked(current: Provider | null, target: Provider, caps: (provider: Provider) => Capabilities): boolean {
  return current != null && current !== target && !caps(target).providerHandoff;
}

/** The one wording for a provider a started chat cannot move to. */
export const SWITCH_LOCKED = "start a new chat to switch";

/** What a send uses: the composer names all three, never a default. */
export interface Selection {
  provider: Provider;
  model: string | null;
  effort: string | null;
}

/** The selection a pick makes. Providers share no model ids or levels, so
 *  a new provider starts with neither, for the composer to fill from that
 *  provider's catalogue as it does a new chat's. */
export function pickSelection(current: Selection, change: Partial<Selection>): Selection {
  if (change.provider && change.provider !== current.provider) {
    return { provider: change.provider, model: change.model ?? null, effort: change.effort ?? null };
  }
  return {
    provider: current.provider,
    model: change.model !== undefined ? change.model : current.model,
    effort: change.effort !== undefined ? change.effort : current.effort,
  };
}

/** Whether a pick is saved (`harnessSelectModel`) now: once it names a
 *  model. A provider whose catalogue has not arrived is held until the
 *  composer fills it, so Rust never stores a chat with no model. A running
 *  turn changes nothing here: the pick is stored for the next send, and the
 *  running run keeps what it was sent with. */
export const pickSaves = (next: Selection): boolean => next.model != null;

/** The open chat's selection as the composer shows it: a pick not yet
 *  saved, else the thread's. A thread from before effort was stored reads
 *  the session's reasoning dial. */
export function threadSelection(
  thread: Pick<HarnessThread, "provider" | "model" | "effort">,
  held: Selection | undefined,
  dial: string | null,
): Selection {
  return held ?? { provider: thread.provider, model: thread.model, effort: thread.effort ?? dial };
}

/** The provider an open chat is moving away from with its next send: the
 *  newest run's, when the selection names another. Null with no switch
 *  waiting, and before the chat's first recorded run. */
export function pendingSwitch(selection: Selection, runs: Record<number, HarnessRun> | undefined): Provider | null {
  let newest: HarnessRun | undefined;
  for (const run of Object.values(runs ?? {})) if (run.provider && (!newest || run.id > newest.id)) newest = run;
  return newest?.provider && newest.provider !== selection.provider ? newest.provider : null;
}

/** What a run was sent with that the run before it was not: the sides of a
 *  `{from} → {to}` divider, each only where it changed. A model the CLI
 *  resolved differently is not a change of the student's, and a level is
 *  compared only where both runs recorded one (runs from before levels were
 *  stored have none). Null when the two runs match, and when either is
 *  from before selections were recorded. */
export interface RunSwitch {
  provider?: [Provider, Provider];
  model?: [string | null, string | null];
  effort?: [string, string];
}

export function runSwitch(before: HarnessRun | undefined, run: HarnessRun): RunSwitch | null {
  if (!before?.provider || !run.provider) return null;
  const change: RunSwitch = {};
  if (before.provider !== run.provider) change.provider = [before.provider, run.provider];
  if ((before.model ?? null) !== (run.model ?? null)) change.model = [before.model ?? null, run.model ?? null];
  if (before.effort && run.effort && before.effort !== run.effort) change.effort = [before.effort, run.effort];
  return Object.keys(change).length ? change : null;
}

/** A model's display name; the raw id where no catalogue names it. */
export type ModelName = (provider: Provider, model: string) => string;

/** A divider's two sides, naming only what changed: the agent where it
 *  changed, the model, and the level ("High reasoning" when the level is
 *  all that changed). `owners` are the two runs' providers, whose
 *  catalogues name their models. */
export function switchSides(change: RunSwitch, name: ModelName, owners: [Provider, Provider]): [string, string] {
  const side = (i: 0 | 1) => {
    const provider = change.provider?.[i];
    const model = change.model?.[i];
    const effort = change.effort?.[i];
    const parts = [
      provider && providerLabel(provider),
      model && name(owners[i], model),
      effort && (change.model || change.provider ? reasoningLabel(effort) : `${reasoningLabel(effort)} reasoning`),
    ].filter(Boolean);
    return parts.join(" · ") || "—";
  };
  return [side(0), side(1)];
}

/** A handoff divider's words and tone: pending while its prompt is on the
 *  way, danger when it was never delivered. */
export function handoffLabel(handoff: RunHandoff): { label: string; tone: "neutral" | "danger" } {
  if (handoff.status === "pending") return { label: "Context handoff pending", tone: "neutral" };
  return { label: "Context handoff", tone: handoff.status === "failed" ? "danger" : "neutral" };
}

/** "3 earlier messages left out"; null when nothing was. */
export function handoffOmitted(handoff: RunHandoff): string | null {
  if (handoff.omitted <= 0) return null;
  return `${handoff.omitted} earlier ${handoff.omitted === 1 ? "message" : "messages"} left out`;
}

/** The divider above a run: the switch from the run before, its handoff, or
 *  both in one line; null for neither. */
export interface RunDivider {
  label: string;
  detail?: string;
  tone: "neutral" | "danger";
  /** On hover: the provider and model the run was sent to. */
  title?: string;
  switched: boolean;
}

export function runDivider(before: HarnessRun | undefined, run: HarnessRun, name: ModelName): RunDivider | null {
  const change = runSwitch(before, run);
  const handoff = run.handoff ? handoffLabel(run.handoff) : null;
  if (!change && !handoff) return null;
  const omitted = run.handoff ? handoffOmitted(run.handoff) : null;
  const title = run.provider ? [providerLabel(run.provider), runModel(run)].filter(Boolean).join(" · ") : undefined;
  if (!change) {
    return { label: handoff!.label, ...(omitted && { detail: omitted }), tone: handoff!.tone, title, switched: false };
  }
  const [from, to] = switchSides(change, name, [before!.provider!, run.provider!]);
  const detail = handoff ? [handoff.label, omitted].filter(Boolean).join(", ") : undefined;
  return { label: `${from} → ${to}`, ...(detail && { detail }), tone: handoff?.tone ?? "neutral", title, switched: true };
}

/** An answer footer's model: its run's (`runModel`), named by the catalogue
 *  where it is known, with the agent, id and level on hover. Null for a run
 *  from before models were recorded, whose footer shows its time alone. */
export function answerModel(run: HarnessRun | undefined, name: ModelName): { label: string; title: string } | null {
  const model = runModel(run);
  if (!run?.provider || !model) return null;
  const level = run.effort ? `${reasoningLabel(run.effort)} reasoning` : null;
  return {
    label: name(run.provider, model),
    title: [providerLabel(run.provider), model, level].filter(Boolean).join(" · "),
  };
}

/** The provider whose `rewind` gates a question's Edit, Rewind and Retry:
 *  its own run's, since Rust rewinds that session. A question with no
 *  recorded provider (rows from before runs) reads the chat's; null when
 *  neither is known. */
export const questionProvider = (run: HarnessRun | undefined, chat: Provider | undefined): Provider | null =>
  run?.provider ?? chat ?? null;
