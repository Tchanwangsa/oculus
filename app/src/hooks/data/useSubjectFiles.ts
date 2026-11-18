import { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { getFilesForSubject, type DbFile } from "@/lib/db";
import { FILE_ACCESSED_EVENT } from "@/lib/files/openFile";
import { WHITEBOARDS_CHANGED_EVENT } from "@/lib/whiteboard/files";
import { DOCUMENTS_CHANGED_EVENT } from "@/lib/notes/documents";
import { DRIVE_CHANGED_EVENT } from "@/lib/drive/listing";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { createPendingReader } from "@/lib/platform/pendingRead";

const readFiles = createPendingReader(getFilesForSubject);

/** The `files.category` values (`category_of` in
 *  `app/src-tauri/src/library/layout/category.rs`), each always listed. */
const KNOWN_CATEGORIES = [
  "home", "syllabus", "module", "page", "file", "upload", "document", "whiteboard", "notebook",
  "announcement", "assignment", "quiz", "ed", "lessons", "image", "other",
] as const;
type KnownCategory = (typeof KNOWN_CATEGORIES)[number];

export function useSubjectFiles(subjectId: number | null) {
  const [files, setFiles] = useState<DbFile[]>([]);
  const [loadedSubjectId, setLoadedSubjectId] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const [failure, setFailure] = useState<{ subjectId: number; error: string } | null>(null);

  const run = useRef(0);
  const load = useCallback(async (id: number, fresh: boolean) => {
    const token = ++run.current;
    setLoading(true);
    setFailure(null);
    try {
      const rows = await readFiles(id, fresh);
      if (token === run.current) { setFiles(rows); setLoadedSubjectId(id); }
      return rows;
    } catch (error) {
      if (token === run.current) setFailure({ subjectId: id, error: String(error) });
      throw error;
    } finally {
      if (token === run.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (subjectId == null) {
      setFiles([]);
      setLoading(false);
      return;
    }
    void load(subjectId, refresh > 0).catch((e) => console.error("read subject files failed", e));
    return () => { run.current++; };
  }, [subjectId, refresh, load]);

  const reload = useCallback(() => setRefresh((r) => r + 1), []);

  // These events change `files` rows (an open stamps last_accessed_at, Drive
  // moves and renames), so every list of a subject's files refetches here
  // rather than page by page.
  useWindowEvent([FILE_ACCESSED_EVENT, DOCUMENTS_CHANGED_EVENT, WHITEBOARDS_CHANGED_EVENT, DRIVE_CHANGED_EVENT], (event) => {
    const changedId = (event as CustomEvent<{ subjectId?: number } | undefined>).detail?.subjectId;
    if (changedId == null || changedId === subjectId) reload();
  });

  // Every category gets a list, the ones pages read always present; one
  // Rust adds later still lands, so no file drops out of a view.
  const byCategory = useMemo(() => {
    const groups: Record<string, DbFile[]> & Record<KnownCategory, DbFile[]> = Object.fromEntries(
      KNOWN_CATEGORIES.map((c) => [c, [] as DbFile[]]),
    ) as Record<KnownCategory, DbFile[]>;
    for (const file of files) (groups[file.category ?? "other"] ??= []).push(file);
    return groups;
  }, [files]);

  return { files, loading, loaded: subjectId != null && loadedSubjectId === subjectId, byCategory, reload,
    error: failure?.subjectId === subjectId ? failure.error : null };
}
