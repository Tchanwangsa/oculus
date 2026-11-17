import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getFilesForSubject, type DbFile } from "@/lib/db";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { createPendingReader } from "@/lib/platform/pendingRead";
import { FILE_ACCESSED_EVENT } from "@/lib/files/openFile";
import { LIBRARY_CHANGE_EVENTS } from "./useFolderListing";

const readFiles = createPendingReader(getFilesForSubject);

const SETTLE_MS = 300;

/**
 * The `files` rows of the subjects on screen, by `relative_path`: what Drive
 * joins onto the disk listing for parse state, recency and categories. Rows
 * are read the way every file list reads them (`getFilesForSubject`), and
 * re-read when a file is opened or the library changes.
 */
export function useDriveRows(subjectIds: readonly number[]): ReadonlyMap<string, DbFile> {
  const key = [...new Set(subjectIds)].sort((a, b) => a - b).join(",");
  const [rows, setRows] = useState<ReadonlyMap<string, DbFile>>(new Map());
  const run = useRef(0);

  const load = useCallback(async (fresh: boolean) => {
    const ids = key ? key.split(",").map(Number) : [];
    const token = ++run.current;
    try {
      const lists = await Promise.all(ids.map((id) => readFiles(id, fresh)));
      if (token !== run.current) return;
      setRows(new Map(lists.flat().map((row) => [row.relative_path, row])));
    } catch (e) {
      console.error("read Drive rows failed", e);
    }
  }, [key]);

  useEffect(() => {
    void load(false);
    return () => { run.current++; };
  }, [load]);

  const settle = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => { if (settle.current) clearTimeout(settle.current); }, []);
  const events = useMemo(() => [FILE_ACCESSED_EVENT, ...LIBRARY_CHANGE_EVENTS], []);
  useWindowEvent(events, () => {
    if (settle.current) clearTimeout(settle.current);
    settle.current = setTimeout(() => { settle.current = null; void load(true); }, SETTLE_MS);
  });

  return rows;
}
