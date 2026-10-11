import { create } from "zustand";
import { getSetting, setSetting } from "@/lib/db";
import { dock, DOCK_SERVICES, isDockServiceId, type DockServiceId } from "@/lib/dock";
import { useDockStore } from "@/stores/shell/dockStore";

/** Which dock services are on: a JSON array of ids in SQLite. Rust keeps no
 *  record, so `useDockPages` re-enables each one at startup. */

const ENABLED_KEY = "dock_enabled";

interface DockPrefs {
  /** In `DOCK_SERVICES` order. */
  enabled: DockServiceId[];
  loaded: boolean;
  load: () => Promise<void>;
  setEnabled: (id: DockServiceId, on: boolean) => Promise<void>;
}

function parse(raw: string | null): DockServiceId[] {
  try {
    const list: unknown = raw ? JSON.parse(raw) : [];
    if (!Array.isArray(list)) return [];
    return DOCK_SERVICES.map((s) => s.id).filter((id) => list.includes(id));
  } catch {
    return [];
  }
}

export const useDockPrefsStore = create<DockPrefs>((set, get) => ({
  enabled: [],
  loaded: false,
  load: async () => {
    const enabled = parse(await getSetting(ENABLED_KEY));
    set({ enabled, loaded: true });
  },
  setEnabled: async (id, on) => {
    if (!isDockServiceId(id)) return;
    const current = get().enabled;
    const enabled = DOCK_SERVICES.map((s) => s.id).filter((s) =>
      s === id ? on : current.includes(s),
    );
    set({ enabled });
    await setSetting(ENABLED_KEY, JSON.stringify(enabled));
    try {
      useDockStore.getState().apply(await dock.setEnabled(id, on));
    } catch (e) {
      console.error(`[oculus] dock: could not ${on ? "open" : "close"} ${id}`, e);
    }
  },
}));
