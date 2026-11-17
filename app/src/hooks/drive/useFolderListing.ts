import { useCallback, useEffect, useRef, useState } from "react";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { DRIVE_CHANGED_EVENT, listFolder, type FolderListing } from "@/lib/drive/listing";
import { UPLOADS_CHANGED_EVENT } from "@/lib/files/uploads";
import { DOCUMENTS_CHANGED_EVENT } from "@/lib/notes/documents";
import { CANVASES_CHANGED_EVENT } from "@/lib/canvas/files";
import { FILE_SCRAPED_EVENT } from "@/lib/pipeline/syncRunner";

/** Events after which a folder on screen may hold something else. */
export const LIBRARY_CHANGE_EVENTS = [
  DRIVE_CHANGED_EVENT,
  UPLOADS_CHANGED_EVENT,
  DOCUMENTS_CHANGED_EVENT,
  CANVASES_CHANGED_EVENT,
  FILE_SCRAPED_EVENT,
] as const;

/** A sync raises one event per file; the listing reads the disk once they
 *  stop coming this long. */
const SETTLE_MS = 300;

/**
 * One folder's listing (`listFolder`), re-read when the library changes. A
 * reply for a folder that is no longer the one asked for is dropped. `null`
 * skips the read (a pane that has nothing to list yet).
 */
export function useFolderListing(rel: string | null, recursive = false) {
  const key = `${recursive ? "all" : "one"}:${rel}`;
  const [held, setHeld] = useState<{ key: string; listing: FolderListing } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const run = useRef(0);

  const load = useCallback(async () => {
    if (rel == null) return;
    const token = ++run.current;
    setLoading(true);
    try {
      const next = await listFolder(rel, recursive);
      if (token !== run.current) return;
      setHeld({ key: `${recursive ? "all" : "one"}:${rel}`, listing: next });
      setError(null);
    } catch (e) {
      if (token !== run.current) return;
      setError(String(e));
    } finally {
      if (token === run.current) setLoading(false);
    }
  }, [rel, recursive]);

  useEffect(() => {
    void load();
    return () => { run.current++; };
  }, [load]);

  const settle = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => { if (settle.current) clearTimeout(settle.current); }, []);
  useWindowEvent(LIBRARY_CHANGE_EVENTS, () => {
    if (settle.current) clearTimeout(settle.current);
    settle.current = setTimeout(() => { settle.current = null; void load(); }, SETTLE_MS);
  });

  // A listing of another folder (or the other mode) is never shown as this one.
  const listing = held?.key === key ? held.listing : null;
  return { listing, error, loading, reload: load };
}
