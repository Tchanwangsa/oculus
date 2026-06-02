import { useState, useEffect, useMemo, useCallback } from "react";
import { getSubjects, type Subject } from "@/lib/db";

export function useSubjects() {
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const rows = await getSubjects();
      setSubjects(rows);
      return rows;
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const current = useMemo(
    () => subjects.filter((s) => s.is_current),
    [subjects],
  );

  const past = useMemo(
    () => subjects.filter((s) => !s.is_current),
    [subjects],
  );

  return { subjects, loading, current, past, reload: load };
}
