import { create } from "zustand";

import type { PastedText } from "@/lib/attachments";

/** One string per composer: an empty box is no entry. */
type Drafts = Record<string, string>;

/** Long pastes held as cards, per composer: none is no entry. */
type Pastes = Record<string, PastedText[]>;

const KEY = "oculus.chatDrafts";
const PASTES_KEY = "oculus.chatPastes";

/** A composer's key: its thread, or the not-yet-thread it would start
 *  (`scope` tells the Chat page's new thread from each lecture's). */
export function draftKey(threadId: number | null, scope: string): string {
  return threadId == null ? `new:${scope}` : `thread:${threadId}`;
}

/** A stored map, keeping only the entries `valid` accepts. */
function read<T>(key: string, valid: (v: unknown) => v is T): Record<string, T> {
  try {
    const raw = localStorage.getItem(key);
    const parsed = raw ? (JSON.parse(raw) as unknown) : {};
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).filter((e): e is [string, T] => valid(e[1])));
  } catch {
    return {};
  }
}

function write(key: string, map: Record<string, unknown>): void {
  try {
    localStorage.setItem(key, JSON.stringify(map));
  } catch {
    /* quota or private mode — the in-memory copy still covers a thread switch */
  }
}

const isString = (v: unknown): v is string => typeof v === "string";

const isPastes = (v: unknown): v is PastedText[] =>
  Array.isArray(v) &&
  v.every(
    (p) =>
      p != null &&
      typeof p === "object" &&
      typeof (p as PastedText).id === "string" &&
      typeof (p as PastedText).text === "string",
  );

interface DraftState {
  drafts: Drafts;
  pastes: Pastes;
  setDraft: (key: string, text: string) => void;
  setPastes: (key: string, list: PastedText[]) => void;
}

/**
 * Unsent words per composer, so switching thread, tab or lecture (which
 * unmounts a composer) never loses them. In localStorage as well, because a
 * `tauri dev` rebuild relaunches the app. Long pastes held as cards
 * (`PastedText`) are kept the same way, under the same key.
 */
export const useDraftStore = create<DraftState>((set, get) => ({
  drafts: read(KEY, isString),
  pastes: read(PASTES_KEY, isPastes),
  setDraft: (key, text) => {
    const prev = get().drafts;
    if ((prev[key] ?? "") === text) return;
    const next = { ...prev };
    if (text) next[key] = text;
    else delete next[key];
    write(KEY, next);
    set({ drafts: next });
  },
  setPastes: (key, list) => {
    const prev = get().pastes;
    if (!list.length && !prev[key]) return;
    const next = { ...prev };
    if (list.length) next[key] = list;
    else delete next[key];
    write(PASTES_KEY, next);
    set({ pastes: next });
  },
}));
