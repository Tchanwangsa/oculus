import { useCallback, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useSubjects } from "@/hooks/data/useSubjects";
import { DEFAULT_FILTER, type TaskFilter } from "./filterRules";
import { parseSubjectParam } from "./taskViews";

/** The Subject filter is URL state so subject navigation, history and restored tabs agree. */
export function useTaskFilterState(): [TaskFilter, (next: TaskFilter) => void] {
  const [params, setParams] = useSearchParams();
  const { subjects } = useSubjects();
  const scope = parseSubjectParam(params.get("subject"));
  const [picked, setPicked] = useState({ value: DEFAULT_FILTER, scope });
  // A subject still loading (or gone) must not briefly show another subject's work.
  const subject = scope === "personal" ? "personal" : scope == null ? "any"
    : subjects.find(s => s.id === scope)?.code ?? "Unknown subject";
  const filter = useMemo<TaskFilter>(() => ({ ...picked.value, project: picked.scope === scope ? picked.value.project : "any", subject }), [picked, scope, subject]);
  const change = useCallback((next: TaskFilter) => {
    setPicked({ value: next, scope });
    if (next.subject === filter.subject) return;
    const query = new URLSearchParams(params);
    const id = subjects.find(s => s.code === next.subject)?.id;
    if (next.subject === "personal") query.set("subject", "personal");
    else if (id != null) query.set("subject", String(id));
    else query.delete("subject");
    setParams(query, { replace: true });
  }, [params, setParams, subjects, filter.subject, scope]);
  return [filter, change];
}
