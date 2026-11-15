import { create } from "zustand";

import type { Subject } from "@/lib/db";
import type { PastedText } from "@/lib/harness/attachments";
import { stripControlChars } from "@/lib/harness/controlChars";
import { isChatScope, liveScope, type ChatScope } from "@/lib/harness/scope";

/** One string per composer: an empty box is no entry. */
type Drafts = Record<string, string>;

/** Long pastes held as cards, per composer: none is no entry. */
type Pastes = Record<string, PastedText[]>;

/** A new thread's chosen scope, per composer: unchosen is no entry. */
type Scopes = Record<string, ChatScope>;

/** Pictures attached to a draft, as `./attachments/…` paths: the shelf of
 *  thumbnails. A PDF or text file is only its chip, so it is never here. */
type Shelves = Record<string, string[]>;

const KEY = "oculus.chatDrafts";
const PASTES_KEY = "oculus.chatPastes";
const SCOPES_KEY = "oculus.chatScopes";
const SHELVES_KEY = "oculus.chatShelves";

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

/** The scopes as storage holds them, damaged entries dropped: what a launch reads. */
export const storedScopes = (): Scopes => read(SCOPES_KEY, isChatScope);

const isPastes = (v: unknown): v is PastedText[] =>
  Array.isArray(v) &&
  v.every(
    (p) =>
      p != null &&
      typeof p === "object" &&
      typeof (p as PastedText).id === "string" &&
      typeof (p as PastedText).text === "string",
  );

const isStrings = (v: unknown): v is string[] => Array.isArray(v) && v.every(isString);

interface DraftState {
  drafts: Drafts;
  pastes: Pastes;
  scopes: Scopes;
  shelves: Shelves;
  setDraft: (key: string, text: string) => void;
  setPastes: (key: string, list: PastedText[]) => void;
  setShelf: (key: string, paths: string[]) => void;
  /** The scope chosen for a new thread's composer; null forgets the choice. */
  setScope: (key: string, scope: ChatScope | null) => void;
  /** That choice, if it still names General or a subject in `subjects`. */
  scopeFor: (key: string, subjects: readonly Pick<Subject, "id">[]) => ChatScope | null;
}

/**
 * Unsent words per composer, so switching thread, tab or lecture (which
 * unmounts a composer) never loses them. In localStorage as well, because a
 * `tauri dev` rebuild relaunches the app. Long pastes held as cards
 * (`PastedText`) are kept the same way, under the same key, and so are the
 * pictures on its shelf (written to `agents/attachments/` when attached) and
 * the scope a new Chat thread will start in (`ChatScope`), until it is sent.
 */
export const useDraftStore = create<DraftState>((set, get) => ({
  drafts: Object.fromEntries(
    Object.entries(read(KEY, isString))
      .map(([key, text]) => [key, stripControlChars(text)])
      .filter(([, text]) => text),
  ),
  pastes: read(PASTES_KEY, isPastes),
  scopes: storedScopes(),
  shelves: read(SHELVES_KEY, isStrings),
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
  setShelf: (key, paths) => {
    const prev = get().shelves;
    if (!paths.length && !prev[key]) return;
    const next = { ...prev };
    if (paths.length) next[key] = paths;
    else delete next[key];
    write(SHELVES_KEY, next);
    set({ shelves: next });
  },
  setScope: (key, scope) => {
    const prev = get().scopes;
    if (prev[key] === (scope ?? undefined)) return;
    const next = { ...prev };
    if (scope != null) next[key] = scope;
    else delete next[key];
    write(SCOPES_KEY, next);
    set({ scopes: next });
  },
  scopeFor: (key, subjects) => liveScope(get().scopes[key], subjects),
}));
