import { create } from "zustand";

const KEY = "oculus.chatWidgets";

/** The student's answer to a widget's `oculus/ask` (docs/widgets.md): whether
 *  it may use a model, and how many requests this allowance has spent. */
export type AskGrant = { allowed: boolean; used: number };
export type Entry = { source: string; state: Record<string, unknown>; ask?: AskGrant };
type Entries = Record<string, Entry>;
type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;

export const widgetKey = (thread: number, row: number, block: number) => `${thread}:${row}:${block}`;

/** A saved widget on the Artifacts page. Chat keys start with a thread
 *  number and this can't, so `prune` never reaches it. */
export const artifactWidgetKey = (id: number) => `artifact:${id}`;

/** The state an entry keeps for `source` — never its grant, which stays
 *  with this entry: a widget saved elsewhere asks the student again. */
export const entryState = (entry: Entry | undefined, source: string) => (entry?.source === source ? entry.state : {});

const isGrant = (v: unknown): v is AskGrant => {
  const g = v as Record<string, unknown> | null;
  return !!g && typeof g === "object" && typeof g.allowed === "boolean" && Number.isInteger(g.used) && (g.used as number) >= 0;
};

function read(storage?: Storage): Entries {
  try {
    const data: unknown = JSON.parse(storage?.getItem(KEY) ?? "{}");
    if (!data || typeof data !== "object" || Array.isArray(data)) return {};
    return Object.fromEntries(
      Object.entries(data)
        .filter(([, e]) => e && typeof e.source === "string" && e.state && typeof e.state === "object" && !Array.isArray(e.state))
        // A malformed grant is dropped on its own, so the widget asks again.
        .map(([key, e]) => [key, isGrant(e.ask) ? { source: e.source, state: e.state, ask: e.ask } : { source: e.source, state: e.state }]),
    );
  } catch {
    return {};
  }
}

function browserStorage(): Storage | undefined {
  try { return globalThis.localStorage; } catch { return undefined; }
}

/** Like composer drafts, widget progress is local view state. It outlives
 *  row virtualization and relaunches without changing Rust's chat rows. An
 *  entry belongs to one `source`: a changed source starts with no state and
 *  no grant. */
export function createWidgetStore(storage?: Storage) {
  return create<{
    entries: Entries;
    save: (key: string, source: string, state: Record<string, unknown>) => void;
    setAsk: (key: string, source: string, ask: AskGrant) => void;
    prune: (thread: number, fromRow?: number) => void;
    drop: (key: string) => void;
  }>((set, get) => {
    const commit = (entries: Entries) => {
      try { storage?.setItem(KEY, JSON.stringify(entries)); } catch { /* Keep progress in memory if storage is full. */ }
      set({ entries });
    };
    /** The entry's fields kept for `source`, or none for another. */
    const same = (key: string, source: string) => {
      const e = get().entries[key];
      return e?.source === source ? e : undefined;
    };
    return {
      entries: read(storage),
      save: (key, source, state) => {
        const ask = same(key, source)?.ask;
        commit({ ...get().entries, [key]: ask ? { source, state, ask } : { source, state } });
      },
      setAsk: (key, source, ask) =>
        commit({ ...get().entries, [key]: { source, state: same(key, source)?.state ?? {}, ask } }),
      prune: (thread, fromRow) => commit(Object.fromEntries(
        Object.entries(get().entries).filter(([key]) => {
          const [t, row] = key.split(":").map(Number);
          return t !== thread || (fromRow !== undefined && row > 0 && row < fromRow);
        }),
      )),
      drop: (key) => {
        if (!(key in get().entries)) return;
        const rest = { ...get().entries };
        delete rest[key];
        commit(rest);
      },
    };
  });
}

export const useWidgetStore = createWidgetStore(browserStorage());
