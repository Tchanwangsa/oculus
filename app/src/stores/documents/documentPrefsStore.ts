import { create } from "zustand";

import { getDocumentSuggestions, setDocumentSuggestions } from "@/lib/notes/documents";

/** Note-editor preferences shared by every open note, so the AI-suggestions
 *  toggle in one tab is the toggle in all of them. Off until loaded. */
interface DocumentPrefs {
  suggestions: boolean;
  loaded: boolean;
  load: () => Promise<void>;
  setSuggestions: (on: boolean) => Promise<void>;
}

export const useDocumentPrefsStore = create<DocumentPrefs>((set, get) => ({
  suggestions: false,
  loaded: false,
  load: async () => {
    if (get().loaded) return;
    const on = await getDocumentSuggestions().catch(() => false);
    // A click while the read was out wins.
    if (!get().loaded) set({ suggestions: on, loaded: true });
  },
  setSuggestions: async (on) => {
    set({ suggestions: on, loaded: true });
    await setDocumentSuggestions(on);
  },
}));
