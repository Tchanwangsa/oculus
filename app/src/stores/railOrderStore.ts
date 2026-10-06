import { create } from "zustand";

/** The sidebar rail's reorderable sections; Search, Sync and Settings stay put. */
export const RAIL_SECTIONS = ["home", "chat", "calendar", "tasks", "subjects"] as const;
export type RailSection = (typeof RAIL_SECTIONS)[number];

const KEY = "oculus-rail-order";

/** Read tolerantly: unknown and duplicate entries are dropped, and a section
 *  the stored order predates is appended. */
function load(): RailSection[] {
  const out: RailSection[] = [];
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (Array.isArray(stored)) {
      for (const v of stored) {
        if (RAIL_SECTIONS.includes(v) && !out.includes(v)) out.push(v);
      }
    }
  } catch {
    /* ignore */
  }
  for (const s of RAIL_SECTIONS) if (!out.includes(s)) out.push(s);
  return out;
}

interface RailOrderState {
  order: RailSection[];
  setOrder: (order: RailSection[]) => void;
}

/** The order the user dragged the rail into, kept across launches. */
export const useRailOrder = create<RailOrderState>((set) => ({
  order: load(),
  setOrder: (order) => {
    set({ order });
    try {
      localStorage.setItem(KEY, JSON.stringify(order));
    } catch {
      /* ignore */
    }
  },
}));
