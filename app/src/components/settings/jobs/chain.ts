import type { JobChain, JobSelection } from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";

const sameModel = (a: JobSelection, b: JobSelection) =>
  a.provider === b.provider && a.model === b.model;

/** Puts `pick` at `index` (`chain.length` appends). A chain never names the
 *  same agent and model twice, so another entry with that model is removed:
 *  the pick wins where it was made, which makes picking a model already lower
 *  in the chain a move. */
export function withEntry(chain: JobChain, index: number, pick: JobSelection): JobChain {
  const next = [...chain];
  next.splice(index, index < chain.length ? 1 : 0, pick);
  return next.filter((entry, i) => i === index || !sameModel(entry, pick));
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
