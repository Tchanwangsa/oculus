import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCalendarEvents, type DbCalendarEvent, type DbFile } from "@/lib/db";
import { createCourseFileDataLoader } from "@/lib/files/courseFiles";
import { getAllOpenTasks, PROJECTS_UPDATED_EVENT, type DbOpenTask } from "@/lib/planning/projects";
import { CALENDAR_UPDATED_EVENT } from "@/lib/planning/calendar/model";
import { getItems, REVISION_UPDATED_EVENT, type RevisionItem } from "@/lib/revision";
import type { HarnessThread } from "@/lib/harness/types";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { loadSubjectThreads, parseOverviewFile, type FileMetadata } from "./rows";

interface OverviewRows {
  calendar: DbCalendarEvent[];
  tasks: DbOpenTask[];
  revision: RevisionItem[];
  /** Every thread of the subject, read in pages. */
  subjectThreads: HarnessThread[];
  metadata: FileMetadata[];
}

const EMPTY: OverviewRows = { calendar: [], tasks: [], revision: [], subjectThreads: [], metadata: [] };

export function useOverviewRows(subjectId: number, files: DbFile[], filesReady: boolean) {
  const [state, setState] = useState({ subjectId, rows: EMPTY, loading: true, error: null as string | null });
  const run = useRef(0);
  const loadMetadata = useMemo(() => createCourseFileDataLoader(parseOverviewFile), [subjectId]);
  const reload = useCallback(async () => {
    const token = ++run.current;
    if (!filesReady) return;
    setState((previous) => ({ subjectId, rows: previous.subjectId === subjectId ? previous.rows : EMPTY, loading: true, error: null }));
    try {
      const [calendar, tasks, revision, subjectThreads, metadata] = await Promise.all([
        getCalendarEvents(), getAllOpenTasks(), getItems(subjectId), loadSubjectThreads(subjectId),
        loadMetadata(files.filter((file) => ["assignment", "quiz", "ed"].includes(file.category ?? ""))),
      ]);
      if (token === run.current) setState({
        subjectId, rows: { calendar: calendar.filter((row) => row.subject_id === subjectId), tasks, revision, subjectThreads, metadata },
        loading: false, error: null,
      });
    } catch (error) {
      if (token === run.current) setState((previous) => ({ ...previous, loading: false, error: String(error) }));
    }
  }, [subjectId, files, filesReady, loadMetadata]);

  useEffect(() => {
    void reload();
    return () => { run.current++; };
  }, [reload]);
  useWindowEvent([CALENDAR_UPDATED_EVENT, PROJECTS_UPDATED_EVENT, REVISION_UPDATED_EVENT], () => { void reload(); });
  return {
    rows: state.subjectId === subjectId ? state.rows : EMPTY,
    loading: !filesReady || state.subjectId !== subjectId || state.loading,
    error: state.subjectId === subjectId ? state.error : null,
  };
}
