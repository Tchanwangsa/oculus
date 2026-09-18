import { create } from "zustand";
import { getSetting, setSetting } from "@/lib/db";
import {
  SEARCH_ENGINES,
  setOpenLinksInSystem,
  setSearchEngine,
} from "@/lib/browser";

/**
 * The two things about the browser that are a preference rather than a fact:
 * which engine the address bar searches with, and whether a link should open
 * here at all.
 *
 * Both are also needed **synchronously**, from a click handler or a render —
 * `normalizeAddress` is called while you type and `openExternal` from a
 * capture-phase click — and the values live in SQLite. So this store is the
 * loader and the writer, and `lib/browser.ts` keeps the answer in a module
 * value it pushes down on every change. That is the whole reason for the
 * split: the lib may not import this store (`search.ts` imports the lib, and
 * the store imports the lib), and nothing that has to answer in the same tick
 * may await a database read.
 */

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
