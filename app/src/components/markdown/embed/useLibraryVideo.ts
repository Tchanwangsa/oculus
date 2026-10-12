import { useCallback, useEffect, useState } from "react";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { getFileByCanvasId, type DbFile } from "@/lib/db";
import { FILE_SCRAPED_EVENT, type FileScraped } from "@/lib/pipeline/syncRunner";

/**
 * The library's row for a Canvas file, for a `<canvas-video>` card: undefined
 * while looking, null when it isn't downloaded. Looked up again whenever a
 * row for this file is written — a download from this card, another card or
 * the Modules page — and on `recheck`, which Play calls before opening.
 */
export function useLibraryVideo(subjectId: number, canvasId: number) {
  const [file, setFile] = useState<DbFile | null | undefined>(undefined);

  const recheck = useCallback(async () => {
    const found = await getFileByCanvasId(subjectId, canvasId).catch(() => null);
    setFile(found);
    return found;
  }, [subjectId, canvasId]);

  useEffect(() => {
    setFile(undefined);
    void recheck();
  }, [recheck]);

  useWindowEvent(FILE_SCRAPED_EVENT, (e) => {
    const { subject_id, canvas_id } = (e as CustomEvent<FileScraped>).detail;
    if (subject_id === subjectId && canvas_id === canvasId) void recheck();
  });

  return { file, recheck };
}
