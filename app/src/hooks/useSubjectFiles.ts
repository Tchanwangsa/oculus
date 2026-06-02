import { useState, useEffect, useMemo, useCallback } from "react";
import { getFilesForSubject, type DbFile } from "@/lib/db";

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

  const byCategory = useMemo(
    () => ({
      home: files.filter((f) => f.category === "home"),
      module: files.filter((f) => f.category === "module"),
      page: files.filter((f) => f.category === "page"),
      file: files.filter((f) => f.category === "file"),
      announcement: files.filter((f) => f.category === "announcement"),
      image: files.filter((f) => f.category === "image"),
      syllabus: files.filter((f) => f.category === "syllabus"),
    }),
    [files],
  );

  return { files, loading, byCategory, reload };
}
