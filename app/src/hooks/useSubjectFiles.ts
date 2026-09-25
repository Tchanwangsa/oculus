import { useState, useEffect, useMemo, useCallback } from "react";
import { getFilesForSubject, type DbFile } from "@/lib/db";
import { FILE_ACCESSED_EVENT } from "@/lib/openFile";
import { DOCUMENTS_CHANGED_EVENT } from "@/lib/documents";

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

  // Opening a file stamps last_accessed_at; refetch so its "new" dot clears
  // and the accessed time updates in place. A document being created, renamed
  // or deleted is a change to the same table, and every list of a subject's
  // files — including the full page holding the file that just moved — has to
  // see it, so it is answered here rather than page by page.
  useEffect(() => {
    window.addEventListener(FILE_ACCESSED_EVENT, reload);
    window.addEventListener(DOCUMENTS_CHANGED_EVENT, reload);
    return () => {
      window.removeEventListener(FILE_ACCESSED_EVENT, reload);
      window.removeEventListener(DOCUMENTS_CHANGED_EVENT, reload);
    };
  }, [reload]);

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
