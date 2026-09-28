import { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { getSubjects, type Subject } from "@/lib/db";
import { useSyncStore } from "@/stores/syncStore";

export function useSubjects() {
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [loading, setLoading] = useState(true);
  const completedAt = useSyncStore((s) => s.completedAt);
  const loadVersion = useRef(0);

  const load = useCallback(async () => {
    const version = ++loadVersion.current;
    setLoading(true);
    try {
      const rows = await getSubjects();
      if (version === loadVersion.current) setSubjects(rows);
      return rows;
    } finally {
      if (version === loadVersion.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    load().catch((error) => console.error("Could not load subjects", error));
  }, [load, completedAt]);

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
