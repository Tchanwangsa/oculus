import type { JobChain, JobSelection } from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";

/** An agent's models draw on one usage limit, so a second model of the same
 *  agent cannot take over once the first has run out. opencode is the
 *  exception: it fronts many providers, each with a limit of its own. Mirrors
 *  `shares_usage` in `app/src-tauri/src/harness/jobs.rs`. */
export const sharesUsage = (provider: Provider) => provider !== "opencode";

/** Two entries a chain may not both hold: one agent twice, or for opencode
 *  the same model twice. */
const clash = (a: JobSelection, b: JobSelection) =>
  a.provider === b.provider && (sharesUsage(a.provider) || a.model === b.model);

/** Puts `pick` at `index` (`chain.length` appends). The pick wins where it
 *  was made: an entry it clashes with is removed, so picking a model already
 *  lower in the chain is a move. */
export function withEntry(chain: JobChain, index: number, pick: JobSelection): JobChain {
  const next = [...chain];
  next.splice(index, index < chain.length ? 1 : 0, pick);
  return next.filter((entry, i) => i === index || !clash(entry, pick));
}

/** The agents entry `except` may take: not one another entry already holds,
 *  unless its models don't share usage. `-1` is a new entry. */
export function providersFor(
  providers: PickerProvider[],
  chain: JobChain,
  except = -1,
): PickerProvider[] {
  const held = new Set(
    chain.filter((e, i) => i !== except && sharesUsage(e.provider)).map((e) => e.provider),
  );
  return providers.filter((p) => !held.has(p.id));
}

export function withoutEntry(chain: JobChain, index: number): JobChain {
  return chain.filter((_, i) => i !== index);
}

/** Swaps the entry at `index` with its neighbour `by` places away; a move
 *  past either end leaves the chain as it is. */
export function moveEntry(chain: JobChain, index: number, by: -1 | 1): JobChain {
  const to = index + by;
  if (to < 0 || to >= chain.length) return chain;
  const next = [...chain];
  [next[index], next[to]] = [next[to], next[index]];
  return next;
}

/** The agent's display name, else its id. */
export function agentLabel(providers: PickerProvider[], provider: Provider): string {
  return providers.find((p) => p.id === provider)?.label ?? provider;
}

/** The model's display name from its agent's fetched list, else its id. */
export function modelLabel(
  providers: PickerProvider[],
  provider: Provider,
  model: string,
): string {
  const p = providers.find((x) => x.id === provider);
  return p?.models.find((m) => m.id === model)?.label ?? model;
}
