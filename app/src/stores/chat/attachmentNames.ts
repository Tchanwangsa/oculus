import { create } from "zustand";

import { attachmentPath, isImagePath } from "@/lib/harness/attachments";

/** The most names kept; the oldest go first. */
export const MAX_NAMES = 500;

const KEY = "oculus.chatAttachmentNames";

type Names = Record<string, string>;

function read(): Names {
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? "{}") as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).filter((e): e is [string, string] => typeof e[1] === "string"));
  } catch {
    return {};
  }
}

function write(names: Names): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(names));
  } catch {
    /* quota or private mode — the in-memory copy still names them this session */
  }
}

/** `names` with `path` named `name`, newest last, the oldest dropped past `max`. */
export function addName(names: Names, path: string, name: string, max = MAX_NAMES): Names {
  const next = { ...names };
  delete next[path];
  next[path] = name;
  const keys = Object.keys(next);
  for (const key of keys.slice(0, Math.max(0, keys.length - max))) delete next[key];
  return next;
}

/** What a chip says when the file's own name is not known here: its kind. */
export function attachmentKindLabel(path: string): string {
  if (isImagePath(path)) return "Picture";
  return /\.pdf$/i.test(path) ? "PDF" : "Text file";
}

/**
 * The name each attachment had when the student attached it, by its
 * data-directory-relative path. Display only: the file on disk is a stamp and
 * the message carries only the path, so a chip reads its label here, and a
 * path this install never named reads as its kind.
 */
export const useAttachmentNames = create<{
  names: Names;
  name: (raw: string, name: string) => void;
}>((set, get) => ({
  names: read(),
  name: (raw, name) => {
    const path = attachmentPath(raw);
    if (!path || !name || get().names[path] === name) return;
    const names = addName(get().names, path, name);
    write(names);
    set({ names });
  },
}));

/** The label for an attachment in either spelling. */
export function useAttachmentLabel(raw: string): string {
  const path = attachmentPath(raw) ?? raw;
  const name = useAttachmentNames((s) => s.names[path]);
  return name ?? attachmentKindLabel(path);
}
