import { create } from "zustand";
import { getSetting, setSetting } from "@/lib/db";
import {
  SEARCH_ENGINES,
  setOpenLinksInSystem,
  setSearchEngine,
} from "@/lib/browser";

/** Browser preferences: search engine and where links open. Loaded from
 *  SQLite here and pushed into `lib/browser/index.ts` module values, since its
 *  callers answer synchronously and it cannot import this store. */

const ENGINE_KEY = "browser_search_engine";
const OPEN_LINKS_KEY = "browser_open_links_in";

export type OpenLinksIn = "oculus" | "system";

interface BrowserPrefs {
  engine: string;
  openLinksIn: OpenLinksIn;
  loaded: boolean;
  load: () => Promise<void>;
  setEngine: (id: string) => Promise<void>;
  setOpenLinksIn: (where: OpenLinksIn) => Promise<void>;
}

export const useBrowserPrefsStore = create<BrowserPrefs>((set) => ({
  engine: SEARCH_ENGINES[0].id,
  openLinksIn: "oculus",
  loaded: false,
  load: async () => {
    const [engine, openLinks] = await Promise.all([
      getSetting(ENGINE_KEY),
      getSetting(OPEN_LINKS_KEY),
    ]);
    const id = SEARCH_ENGINES.find((e) => e.id === engine)?.id ?? SEARCH_ENGINES[0].id;
    const where: OpenLinksIn = openLinks === "system" ? "system" : "oculus";
    setSearchEngine(id);
    setOpenLinksInSystem(where === "system");
    set({ engine: id, openLinksIn: where, loaded: true });
  },
  setEngine: async (id) => {
    setSearchEngine(id);
    set({ engine: id });
    await setSetting(ENGINE_KEY, id);
  },
  setOpenLinksIn: async (where) => {
    setOpenLinksInSystem(where === "system");
    set({ openLinksIn: where });
    await setSetting(OPEN_LINKS_KEY, where);
  },
}));
