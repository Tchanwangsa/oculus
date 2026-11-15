import { create } from "zustand";

import { attachmentPath } from "@/lib/harness/attachments";
import { attachmentKindLabel } from "@/stores/chat/attachmentNames";

/** The most prompts the stash keeps; a 21st pushes the oldest out. */
export const MAX_STASH = 20;

/** A draft set aside with ⌘S, to restore into any composer later. */
export interface StashEntry {
  id: string;
  /** Epoch ms. */
  createdAt: number;
  /** The box's text as the draft holds it (chips as paths and `/name`). */
  text: string;
  /** Long pastes that were cards, in order. */
  pastes: string[];
  /** Pictures, written to `agents/attachments/` when stashed: each the
   *  `./attachments/<name>` path a message would carry. */
  pictures: string[];
}

const KEY = "oculus.chatStash";

let seq = 0;
const nextId = () => `stash-${Date.now()}-${seq++}`;

/** `entry` on top of `entries`, the oldest dropped past `MAX_STASH`; `evicted`
 *  says one was. Newest first. */
export function pushStash(entries: readonly StashEntry[], entry: StashEntry): { entries: StashEntry[]; evicted: boolean } {
  const next = [entry, ...entries];
  return { entries: next.slice(0, MAX_STASH), evicted: next.length > MAX_STASH };
}

const isStrings = (v: unknown): v is string[] => Array.isArray(v) && v.every((x) => typeof x === "string");

function isEntry(v: unknown): v is StashEntry {
  if (!v || typeof v !== "object") return false;
  const e = v as StashEntry;
  return typeof e.id === "string" && typeof e.createdAt === "number" && typeof e.text === "string" && isStrings(e.pastes) && isStrings(e.pictures);
}

function read(): StashEntry[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? "[]") as unknown;
    return Array.isArray(parsed) ? parsed.filter(isEntry).slice(0, MAX_STASH) : [];
  } catch {
    return [];
  }
}

function write(entries: StashEntry[]): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(entries));
  } catch {
    /* quota or private mode — the in-memory copy still holds them this session */
  }
}

/**
 * Prompts stashed with ⌘S, newest first, shared by every composer and kept in
 * localStorage across relaunches. Pictures are files by then, so an entry is
 * only text and paths. `evicted` stays set until the stash menu closes, so
 * the menu can say the oldest went.
 */
export const useStashStore = create<{
  entries: StashEntry[];
  evicted: boolean;
  stash: (draft: Omit<StashEntry, "id" | "createdAt">) => void;
  remove: (id: string) => void;
  clearEvicted: () => void;
}>((set, get) => ({
  entries: read(),
  evicted: false,
  stash: (draft) => {
    const { entries, evicted } = pushStash(get().entries, { ...draft, id: nextId(), createdAt: Date.now() });
    write(entries);
    set({ entries, evicted: get().evicted || evicted });
  },
  remove: (id) => {
    const entries = get().entries.filter((e) => e.id !== id);
    write(entries);
    set({ entries });
  },
  clearEvicted: () => set({ evicted: false }),
}));

/** An entry's one-line label: its words, each attachment chip as its name
 *  (`label`, else its kind), or what it holds when it has none. */
export function stashSnippet(
  entry: Pick<StashEntry, "text" | "pastes" | "pictures">,
  max = 90,
  label: (path: string) => string | undefined = () => undefined,
): string {
  const named = entry.text.replace(/`([^`\n]+)`/g, (fence, raw: string) => {
    const path = attachmentPath(raw);
    return path ? (label(path) ?? attachmentKindLabel(path)) : fence;
  });
  const words = named.trim().replace(/\s+/g, " ");
  if (words) return words.length > max ? `${words.slice(0, max)}…` : words;
  const parts = [
    entry.pictures.length && `${entry.pictures.length} picture${entry.pictures.length === 1 ? "" : "s"}`,
    entry.pastes.length && `${entry.pastes.length} pasted text${entry.pastes.length === 1 ? "" : "s"}`,
  ].filter(Boolean);
  return parts.length ? `(${parts.join(", ")})` : "(empty)";
}
