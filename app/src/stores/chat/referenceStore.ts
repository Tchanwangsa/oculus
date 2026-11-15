import { useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

import { presentLibraryPaths } from "@/lib/db/files";
import { splitLibraryPaths } from "@/lib/files/libraryPaths";

/** The two lookups a check makes, passed in so tests need no backend. */
export interface ReferenceLookups {
  /** Whether each `agents/attachments/…` file is on disk, in order. */
  attachmentsExist: (paths: string[]) => Promise<boolean[]>;
  /** Which `courses/…` paths the files table still holds. */
  libraryPresent: (paths: string[]) => Promise<Set<string>>;
}

const BACKEND: ReferenceLookups = {
  attachmentsExist: (paths) => invoke<boolean[]>("harness_attachments_exist", { paths }),
  libraryPresent: (paths) => presentLibraryPaths(paths),
};

/** The references a message or draft makes that can go missing: its
 *  attachments and its `@` library paths, data-directory-relative. A path
 *  outside `courses/` (a lecture, an agent's output) is never checked. */
export function checkablePaths(text: string): string[] {
  const out = new Set<string>();
  for (const p of splitLibraryPaths(text)) {
    if (p.kind === "image" || p.kind === "attachment") out.add(p.path);
    else if (p.kind === "path" && p.path.startsWith("courses/")) out.add(p.path);
  }
  return [...out];
}

/** Which of `paths` are gone, in one batched call per kind. A lookup that
 *  fails marks nothing: a reference is shown unresolved only when known gone. */
export async function missingReferences(paths: readonly string[], lookups: ReferenceLookups = BACKEND): Promise<Map<string, boolean>> {
  const result = new Map<string, boolean>();
  const attachments = paths.filter((p) => p.startsWith("agents/attachments/"));
  const library = paths.filter((p) => p.startsWith("courses/"));
  const [exist, present] = await Promise.all([
    attachments.length ? lookups.attachmentsExist(attachments).catch(() => null) : [],
    library.length ? lookups.libraryPresent(library).catch(() => null) : new Set<string>(),
  ]);
  if (exist) attachments.forEach((p, i) => result.set(p, exist[i] === false));
  if (present) library.forEach((p) => result.set(p, !present.has(p)));
  return result;
}

/**
 * Which references are known to be gone, by data-directory-relative path,
 * shared by every bubble and composer. A chip reads its own entry, so marking
 * one redraws that chip alone, never the composer's editable text. Never
 * removed from a message: an unresolved chip says "This context is no longer
 * available."
 */
export const useReferenceStore = create<{
  missing: Record<string, boolean>;
  check: (paths: readonly string[]) => Promise<void>;
}>((set, get) => ({
  missing: {},
  check: async (paths) => {
    if (!paths.length) return;
    const found = await missingReferences(paths);
    const prev = get().missing;
    if ([...found].every(([p, gone]) => (prev[p] ?? false) === gone)) return;
    set({ missing: { ...prev, ...Object.fromEntries(found) } });
  },
}));

/** Whether this reference is known to be gone. */
export function useReferenceMissing(path: string | null): boolean {
  return useReferenceStore((s) => (path ? (s.missing[path] ?? false) : false));
}

/** Checks the text's references when they change (not on every keystroke):
 *  one batched check per bubble or draft. */
export function useReferenceCheck(text: string): void {
  const key = useMemo(() => checkablePaths(text).join("\n"), [text]);
  useEffect(() => {
    if (key) void useReferenceStore.getState().check(key.split("\n"));
  }, [key]);
}
