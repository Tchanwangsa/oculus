import { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { getSubjects, type Subject } from "@/lib/db";
import { createPendingReader } from "@/lib/pendingRead";

const readSubjects = createPendingReader<null, Subject[]>(getSubjects);

export function useSubjects() {
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [loading, setLoading] = useState(true);

  const run = useRef(0);
  const load = useCallback(async (fresh = true) => {
    const token = ++run.current;
    setLoading(true);
    try {
      const rows = await readSubjects(null, fresh);
      if (token === run.current) setSubjects(rows);
      return rows;
    } finally {
      if (token === run.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(false).catch((e) => console.error("read subjects failed", e));
    return () => { run.current++; };
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
