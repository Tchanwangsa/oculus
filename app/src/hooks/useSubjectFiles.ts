import { useState, useEffect, useMemo, useCallback } from "react";
import { getFilesForSubject, type DbFile } from "@/lib/db";
import { FILE_ACCESSED_EVENT } from "@/lib/openFile";
import { DOCUMENTS_CHANGED_EVENT } from "@/lib/documents";
import { useWindowEvent } from "@/hooks/useEvents";

export function useSubjectFiles(subjectId: number | null) {
  const [files, setFiles] = useState<DbFile[]>([]);
  const [loading, setLoading] = useState(false);
  const [refresh, setRefresh] = useState(0);

  const load = useCallback(async (id: number) => {
    setLoading(true);
    try {
      const rows = await getFilesForSubject(id);
      setFiles(rows);
      return rows;
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (subjectId == null) return;
    load(subjectId);
  }, [subjectId, refresh, load]);

  const reload = useCallback(() => setRefresh((r) => r + 1), []);

  // Both events change `files` rows (an open stamps last_accessed_at), so every
  // list of a subject's files refetches here rather than page by page.
  useWindowEvent([FILE_ACCESSED_EVENT, DOCUMENTS_CHANGED_EVENT], reload);

  const byCategory = useMemo(
    () => ({
      home: files.filter((f) => f.category === "home"),
      module: files.filter((f) => f.category === "module"),
      page: files.filter((f) => f.category === "page"),
      file: files.filter((f) => f.category === "file"),
      upload: files.filter((f) => f.category === "upload"),
      document: files.filter((f) => f.category === "document"),
      announcement: files.filter((f) => f.category === "announcement"),
      assignment: files.filter((f) => f.category === "assignment"),
      quiz: files.filter((f) => f.category === "quiz"),
      ed: files.filter((f) => f.category === "ed"),
      image: files.filter((f) => f.category === "image"),
      syllabus: files.filter((f) => f.category === "syllabus"),
    }),
    [files],
  );

  return { files, loading, byCategory, reload };
}
