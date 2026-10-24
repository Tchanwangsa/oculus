import { create } from "zustand";
import type { ProposalState } from "@/lib/harness/proposals";

type Workspace = { thread: number; item: number; selected: string; revision?: number; drafts: Record<string, ProposalState> };
type Entries = Record<string, Workspace>;
type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;
const KEY = "oculus.proposalVersions";

function read(storage?: Storage): Entries {
  try {
    const value = JSON.parse(storage?.getItem(KEY) ?? "{}");
    if (!value || typeof value !== "object" || Array.isArray(value)) return {};
    return Object.fromEntries(Object.entries(value).filter((pair): pair is [string, Workspace] => {
      const [, entry] = pair;
      const row = entry as Workspace | null;
      return !!row && Number.isInteger(row.thread) && Number.isInteger(row.item) && typeof row.selected === "string" && !!row.drafts && typeof row.drafts === "object" && !Array.isArray(row.drafts);
    }));
  } catch { return {}; }
}

/** The active draft is durable in SQLite; parked drafts stay with each version. */
export function createProposalVersionStore(storage?: Storage) {
  return create<{
    entries: Entries;
    select: (id: string, thread: number, item: number, version: string, state: ProposalState) => ProposalState | undefined;
    reset: (id: string, thread: number, item: number, version: string) => void;
    prune: (thread: number, fromItem?: number) => void;
  }>((set, get) => {
    const commit = (entries: Entries) => {
      try { storage?.setItem(KEY, JSON.stringify(entries)); } catch { /* Keep parked drafts in memory if storage is full. */ }
      set({ entries });
    };
    return {
      entries: read(storage),
      select: (id, thread, item, version, state) => {
        const previous = get().entries[id] ?? { thread, item, selected: id, drafts: {} };
        const next = previous.drafts[version];
        commit({ ...get().entries, [id]: { ...previous, selected: version, revision: (previous.revision ?? 0) + 1, drafts: { ...previous.drafts, [previous.selected]: state } } });
        return next;
      },
      reset: (id, thread, item, version) => {
        const previous = get().entries[id] ?? { thread, item, selected: version, drafts: {} };
        const drafts = { ...previous.drafts };
        delete drafts[previous.selected];
        commit({ ...get().entries, [id]: { ...previous, revision: (previous.revision ?? 0) + 1, drafts } });
      },
      prune: (thread, fromItem) => commit(Object.fromEntries(Object.entries(get().entries).filter(([, entry]) => entry.thread !== thread || (fromItem !== undefined && entry.item < fromItem)))),
    };
  });
}

function browserStorage(): Storage | undefined {
  try { return globalThis.localStorage; } catch { return undefined; }
}
export const useProposalVersionStore = createProposalVersionStore(browserStorage());
