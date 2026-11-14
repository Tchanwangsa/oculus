import { create } from "zustand";
import { getSetting, setSetting } from "@/lib/db";
import {
  SEARCH_ENGINES,
  setOpenLinksInSystem,
  setSearchEngine,
} from "@/lib/browser";
import type { LastDestination } from "@/lib/browser/downloads/types";

/** Browser preferences: search engine, where links open, and what the Save
 *  dialog preselects for a download. Loaded from
 *  SQLite here and pushed into `lib/browser/index.ts` module values, since its
 *  callers answer synchronously and it cannot import this store. */

const ENGINE_KEY = "browser_search_engine";
const OPEN_LINKS_KEY = "browser_open_links_in";
const LAST_DESTINATION_KEY = "browser_downloads_last_destination";
const LAST_SUBJECT_KEY = "browser_downloads_last_subject";

export type OpenLinksIn = "oculus" | "system";

interface BrowserPrefs {
  engine: string;
  openLinksIn: OpenLinksIn;
  /** The Save dialog's last destination; preselected next time. */
  lastDestination: LastDestination;
  /** The course code last saved to in Oculus. */
  lastSubject: string | null;
  loaded: boolean;
  load: () => Promise<void>;
  setEngine: (id: string) => Promise<void>;
  setOpenLinksIn: (where: OpenLinksIn) => Promise<void>;
  /** After a Save: `subject` is the code chosen, kept for an Oculus save. */
  rememberDestination: (where: LastDestination, subject?: string) => Promise<void>;
}

export const useBrowserPrefsStore = create<BrowserPrefs>((set) => ({
  engine: SEARCH_ENGINES[0].id,
  openLinksIn: "oculus",
  lastDestination: "oculus",
  lastSubject: null,
  loaded: false,
  load: async () => {
    const [engine, openLinks, lastDestination, lastSubject] = await Promise.all([
      getSetting(ENGINE_KEY),
      getSetting(OPEN_LINKS_KEY),
      getSetting(LAST_DESTINATION_KEY),
      getSetting(LAST_SUBJECT_KEY),
    ]);
    const id = SEARCH_ENGINES.find((e) => e.id === engine)?.id ?? SEARCH_ENGINES[0].id;
    const where: OpenLinksIn = openLinks === "system" ? "system" : "oculus";
    setSearchEngine(id);
    setOpenLinksInSystem(where === "system");
    set({
      engine: id,
      openLinksIn: where,
      lastDestination: lastDestination === "mac" ? "mac" : "oculus",
      lastSubject: lastSubject || null,
      loaded: true,
    });
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
  rememberDestination: async (where, subject) => {
    set(subject ? { lastDestination: where, lastSubject: subject } : { lastDestination: where });
    await setSetting(LAST_DESTINATION_KEY, where);
    if (subject) await setSetting(LAST_SUBJECT_KEY, subject);
  },
}));
