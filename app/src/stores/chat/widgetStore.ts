import { create } from "zustand";

const KEY = "oculus.chatWidgets";
type Entry = { source: string; state: Record<string, unknown> };
type Entries = Record<string, Entry>;
type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;

export const widgetKey = (thread: number, row: number, block: number) => `${thread}:${row}:${block}`;

function read(storage?: Storage): Entries {
  try {
    const data: unknown = JSON.parse(storage?.getItem(KEY) ?? "{}");
    if (!data || typeof data !== "object" || Array.isArray(data)) return {};
    return Object.fromEntries(Object.entries(data).filter(([, e]) =>
      e && typeof e.source === "string" && e.state && typeof e.state === "object" && !Array.isArray(e.state),
    ));
  } catch {
    return {};
  }
}

function browserStorage(): Storage | undefined {
  try { return globalThis.localStorage; } catch { return undefined; }
}

/** Like composer drafts, widget progress is local view state. It outlives
 *  row virtualization and relaunches without changing Rust's chat rows. */
export function createWidgetStore(storage?: Storage) {
  return create<{
    entries: Entries;
    save: (key: string, source: string, state: Record<string, unknown>) => void;
    prune: (thread: number, fromRow?: number) => void;
  }>((set, get) => {
    const commit = (entries: Entries) => {
      try { storage?.setItem(KEY, JSON.stringify(entries)); } catch { /* Keep progress in memory if storage is full. */ }
      set({ entries });
    };
    return {
      entries: read(storage),
      save: (key, source, state) => commit({ ...get().entries, [key]: { source, state } }),
      prune: (thread, fromRow) => commit(Object.fromEntries(
        Object.entries(get().entries).filter(([key]) => {
          const [t, row] = key.split(":").map(Number);
          return t !== thread || (fromRow !== undefined && row > 0 && row < fromRow);
        }),
      )),
    };
  });
}

export const useWidgetStore = createWidgetStore(browserStorage());
