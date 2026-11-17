import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getAllLectures, getCalendarEvents, getSubjects, type DbCalendarEvent, type Lecture, type Subject } from "@/lib/db";
import { LECTURES_CHANGED_EVENT } from "@/lib/lectures";
import { CALENDAR_UPDATED_EVENT } from "@/lib/planning/calendar/model";
import { subjectWeeks, type SubjectWeeks } from "@/lib/weeks";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { useNow } from "@/hooks/ui/useNow";

interface TermRows {
  subjects: Subject[];
  classes: DbCalendarEvent[];
  lectures: Lecture[];
}

export interface SubjectWeeksData extends SubjectWeeks, TermRows {
  loading: boolean;
  error: string | null;
  reload: () => Promise<void>;
}

const EMPTY: TermRows = { subjects: [], classes: [], lectures: [] };

/** Reads one term's inputs; derivation and the current-week clock stay local. */
export function useSubjectWeeks(subjectId: number): SubjectWeeksData {
  const [state, setState] = useState<{ subjectId: number; rows: TermRows; loading: boolean; error: string | null }>({
    subjectId, rows: EMPTY, loading: true, error: null,
  });
  const run = useRef(0);
  const reload = useCallback(async () => {
    const token = ++run.current;
    setState((previous) => ({
      subjectId, rows: previous.subjectId === subjectId ? previous.rows : EMPTY, loading: true, error: null,
    }));
    try {
      const [allSubjects, allCalendar, allLectures] = await Promise.all([
        getSubjects(), getCalendarEvents(), getAllLectures(),
      ]);
      const subject = allSubjects.find((row) => row.id === subjectId);
      const subjects = allSubjects.filter((row) => subject && row.term_name === subject.term_name);
      const ids = new Set(subjects.map((row) => row.id));
      const rows = {
        subjects,
        classes: allCalendar.filter((row) => ids.has(row.subject_id) && row.kind === "class"),
        lectures: allLectures.filter((row) => ids.has(row.subject_id)),
      };
      if (token === run.current) setState({ subjectId, rows, loading: false, error: null });
    } catch (error) {
      if (token === run.current) setState((previous) => ({ ...previous, loading: false, error: String(error) }));
    }
  }, [subjectId]);

  useEffect(() => {
    void reload();
    return () => { run.current++; };
  }, [reload]);
  useWindowEvent([CALENDAR_UPDATED_EVENT, LECTURES_CHANGED_EVENT], () => { void reload(); });

  const now = useNow();
  const rows = state.subjectId === subjectId ? state.rows : EMPTY;
  const strip = useMemo(() => subjectWeeks({ subjectId, ...rows, now }), [subjectId, rows, now]);
  return {
    ...strip, ...rows, reload,
    loading: state.subjectId !== subjectId || state.loading,
    error: state.subjectId === subjectId ? state.error : null,
  };
}
