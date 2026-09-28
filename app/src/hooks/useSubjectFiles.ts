import { useState, useEffect, useMemo, useCallback } from "react";
import { getFilesForSubject, type DbFile } from "@/lib/db";
import { FILE_ACCESSED_EVENT } from "@/lib/openFile";
import { UPLOADS_CHANGED_EVENT } from "@/lib/uploads";
import { useSyncStore } from "@/stores/syncStore";

export function useSubjectFiles(subjectId: number | null) {
  const [files, setFiles] = useState<DbFile[]>([]);
  const [loading, setLoading] = useState(subjectId != null);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const completedAt = useSyncStore((s) => s.completedAt);

  useEffect(() => {
    if (subjectId == null) {
      setFiles([]);
      setLoading(false);
      setError(null);
      return;
    }
    let cancelled = false;
    setFiles((previous) => previous.filter((file) => file.subject_id === subjectId));
    setLoading(true);
    setError(null);
    getFilesForSubject(subjectId)
      .then((rows) => { if (!cancelled) setFiles(rows); })
      .catch((reason) => { if (!cancelled) setError(String(reason)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [subjectId, refresh, completedAt]);

  const reload = useCallback(() => setRefresh((r) => r + 1), []);

  // Opening a file stamps last_accessed_at; refetch so its "new" dot clears
  // and the accessed time updates in place.
  useEffect(() => {
    window.addEventListener(FILE_ACCESSED_EVENT, reload);
    window.addEventListener(UPLOADS_CHANGED_EVENT, reload);
    return () => {
      window.removeEventListener(FILE_ACCESSED_EVENT, reload);
      window.removeEventListener(UPLOADS_CHANGED_EVENT, reload);
    };
  }, [reload]);

  const byCategory = useMemo(
    () => ({
      home: files.filter((f) => f.category === "home"),
      module: files.filter((f) => f.category === "module"),
      page: files.filter((f) => f.category === "page"),
      file: files.filter((f) => f.category === "file"),
      upload: files.filter((f) => f.category === "upload"),
      announcement: files.filter((f) => f.category === "announcement"),
      assignment: files.filter((f) => f.category === "assignment"),
      quiz: files.filter((f) => f.category === "quiz"),
      ed: files.filter((f) => f.category === "ed"),
      image: files.filter((f) => f.category === "image"),
      syllabus: files.filter((f) => f.category === "syllabus"),
    }),
    [files],
  );

  return { files, loading, error, byCategory, reload };
}
