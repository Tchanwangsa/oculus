import { getDb } from "@/lib/db/connection";
import { createPendingReader } from "@/lib/platform/pendingRead";
import type { ProposalState } from "./proposals";

export interface ProposalVersion {
  id: string;
  item: number;
  kind: string;
  state: ProposalState;
  next: string | null;
}

type StoredVersion = { item_id: number; block: number; kind: string; initial_state: string; superseded_by: string | null };

/** Original agent snapshots are already immutable in the proposal rows. */
export const readProposalVersions = createPendingReader(async (thread: number): Promise<ProposalVersion[]> => {
  const db = await getDb();
  const rows = await db.select<StoredVersion[]>(
    "SELECT item_id, block, kind, initial_state, superseded_by FROM chat_proposals WHERE thread_id=$1 ORDER BY item_id, block",
    [thread],
  );
  return rows.map((row) => ({ id: `${row.item_id}.${row.block}`, item: row.item_id, kind: row.kind, state: JSON.parse(row.initial_state), next: row.superseded_by }));
});

/** Follow only committed, reciprocal amendment links of the same card kind. */
export function proposalVersions(all: ProposalVersion[], current: ProposalVersion): ProposalVersion[] {
  const byId = new Map(all.map((version) => [version.id, version]));
  byId.set(current.id, byId.get(current.id) ?? current);
  const chain = [byId.get(current.id)!];
  const seen = new Set([current.id]);
  for (;;) {
    const first = chain[0];
    const previous = typeof first.state.amends === "string" ? byId.get(first.state.amends) : undefined;
    if (!previous || previous.kind !== current.kind || previous.next !== first.id || previous.item >= first.item || seen.has(previous.id)) break;
    chain.unshift(previous);
    seen.add(previous.id);
  }
  for (;;) {
    const last = chain[chain.length - 1];
    const next = last.next ? byId.get(last.next) : undefined;
    if (!next || next.kind !== current.kind || next.state.amends !== last.id || next.item <= last.item || seen.has(next.id)) break;
    chain.push(next);
    seen.add(next.id);
  }
  return chain;
}

/** Restoring content never changes the current card's amendment identity. */
export function restoreProposalVersion(snapshot: ProposalState, initial: ProposalState): ProposalState {
  const { amends: _previous, ...fields } = snapshot;
  return "amends" in initial ? { ...fields, amends: initial.amends } : fields;
}

function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value).filter(([, value]) => value !== undefined).sort(([a], [b]) => a.localeCompare(b)).map(([key, entry]) => [key, canonical(entry)]));
}

export function proposalModified(state: ProposalState, snapshot: ProposalState): boolean {
  const { amends: _stateLink, ...content } = state;
  const { amends: _snapshotLink, ...original } = snapshot;
  return JSON.stringify(canonical(content)) !== JSON.stringify(canonical(original));
}
