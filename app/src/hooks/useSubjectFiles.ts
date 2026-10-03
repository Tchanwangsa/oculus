import { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { getFilesForSubject, type DbFile } from "@/lib/db";
import { FILE_ACCESSED_EVENT } from "@/lib/openFile";
import { DOCUMENTS_CHANGED_EVENT } from "@/lib/documents";
import { useWindowEvent } from "@/hooks/useEvents";
import { createPendingReader } from "@/lib/pendingRead";

const readFiles = createPendingReader(getFilesForSubject);

export function useSubjectFiles(subjectId: number | null) {
  const [files, setFiles] = useState<DbFile[]>([]);
  const [loading, setLoading] = useState(false);
  const [refresh, setRefresh] = useState(0);

  const run = useRef(0);
  const load = useCallback(async (id: number, fresh: boolean) => {
    const token = ++run.current;
    setLoading(true);
    try {
      const rows = await readFiles(id, fresh);
      if (token === run.current) setFiles(rows);
      return rows;
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

  // Both events change `files` rows (an open stamps last_accessed_at), so every
  // list of a subject's files refetches here rather than page by page.
  useWindowEvent([FILE_ACCESSED_EVENT, DOCUMENTS_CHANGED_EVENT], (event) => {
    const changedId = (event as CustomEvent<{ subjectId?: number } | undefined>).detail?.subjectId;
    if (changedId == null || changedId === subjectId) reload();
  });

  const byCategory = useMemo(() => {
    const groups = {
      home: [] as DbFile[], module: [] as DbFile[], page: [] as DbFile[],
      file: [] as DbFile[], upload: [] as DbFile[], document: [] as DbFile[],
      announcement: [] as DbFile[], assignment: [] as DbFile[], quiz: [] as DbFile[],
      ed: [] as DbFile[], image: [] as DbFile[], syllabus: [] as DbFile[],
    };
    for (const file of files) {
      const category = file.category;
      if (category && Object.prototype.hasOwnProperty.call(groups, category)) {
        groups[category as keyof typeof groups].push(file);
      }
    }
    return groups;
  }, [files]);

  return { files, loading, byCategory, reload };
}
