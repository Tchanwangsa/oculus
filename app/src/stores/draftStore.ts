import { create } from "zustand";

/** One string per composer: an empty box is no entry. */
type Drafts = Record<string, string>;

const KEY = "oculus.chatDrafts";

/** A composer's key: its thread, or the not-yet-thread it would start
 *  (`scope` tells the Chat page's new thread from each lecture's). */
export function draftKey(threadId: number | null, scope: string): string {
  return threadId == null ? `new:${scope}` : `thread:${threadId}`;
}

function read(): Drafts {
  try {
    const raw = localStorage.getItem(KEY);
    const parsed = raw ? (JSON.parse(raw) as unknown) : {};
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    return Object.fromEntries(
      Object.entries(parsed).filter((e): e is [string, string] => typeof e[1] === "string"),
    );
  } catch {
    return {};
  }
}

function write(drafts: Drafts): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(drafts));
  } catch {
    /* quota or private mode — the in-memory copy still covers a thread switch */
  }
}

interface DraftState {
  drafts: Drafts;
  setDraft: (key: string, text: string) => void;
}

/**
 * Unsent words per composer, so switching thread, tab or lecture (which
 * unmounts a composer) never loses them. In localStorage as well, because a
 * `tauri dev` rebuild relaunches the app.
 */
export const useDraftStore = create<DraftState>((set, get) => ({
  drafts: read(),
  setDraft: (key, text) => {
    const prev = get().drafts;
    if ((prev[key] ?? "") === text) return;
    const next = { ...prev };
    if (text) next[key] = text;
    else delete next[key];
    write(next);
    set({ drafts: next });
  },
}));
