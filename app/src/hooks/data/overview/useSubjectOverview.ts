import { useCallback, useEffect, useMemo, useState } from "react";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useSubjectWeeks } from "@/hooks/data/useSubjectWeeks";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { useNow } from "@/hooks/ui/useNow";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { FILE_SCRAPED_EVENT, type FileScraped } from "@/lib/pipeline/syncRunner";
import { UPLOADS_CHANGED_EVENT } from "@/lib/files/uploads";
import { LECTURE_PROGRESS_EVENT } from "@/lib/lectures/playback/progress";
import { LECTURE_DOWNLOADED_EVENT } from "@/stores/lectures/lectureDownloadStore";
import { subjectRevisionDue } from "@/lib/revision/due";
import { deriveActivity } from "./activity";
import { deriveComingUp } from "./comingUp";
import { deriveEvents } from "./events";
import { deriveYours } from "./yours";
import { deriveBehind } from "./behind";
import { deriveProgress } from "./progress";
import { readVisit, writeVisit } from "./visit";
import { useOverviewRows } from "./useOverviewRows";
import type { SubjectOverviewData } from "./types";

/** A null week selects the current chip. Visits are stamped only by the page's cleanup. */
export function useSubjectOverview(subjectId: number, weekKey: number | null = null): SubjectOverviewData {
  const filesData = useSubjectFiles(subjectId);
  const weeksData = useSubjectWeeks(subjectId);
  const files = useMemo(() => filesData.files.filter((file) => file.subject_id === subjectId), [filesData.files, subjectId]);
  const { rows, loading, error } = useOverviewRows(subjectId, files, filesData.loaded && !filesData.loading);
  const live = useHarnessStore((state) => state.threads);
  const threadsLoaded = useHarnessStore((state) => state.threadsLoaded);
  const threadError = useThreadLoad(threadsLoaded);
  const now = useNow();
  const sinceLastVisit = useMemo(() => readVisit(subjectId), [subjectId]);
  const markVisited = useCallback(() => writeVisit(subjectId, Date.now()), [subjectId]);
  const weeks = useMemo(() => ({ weeks: weeksData.weeks, current: weeksData.current, source: weeksData.source }),
    [weeksData.weeks, weeksData.current, weeksData.source]);
  const key = weekKey ?? weeks.weeks[weeks.current]?.key ?? null;

  useWindowEvent(FILE_SCRAPED_EVENT, (event) => {
    const changed = (event as CustomEvent<FileScraped>).detail?.subject_id;
    if (changed == null || changed === subjectId) filesData.reload();
  });
  useWindowEvent([LECTURE_PROGRESS_EVENT, LECTURE_DOWNLOADED_EVENT], () => { void weeksData.reload(); });
  useWindowEvent(UPLOADS_CHANGED_EVENT, filesData.reload);

  return useMemo(() => ({
    weeks,
    activity: deriveActivity(subjectId, weeks, key, files, weeksData.lectures, weeksData.classes,
      rows.metadata.flatMap((row) => row.thread ? [row.thread] : []), sinceLastVisit),
    comingUp: deriveComingUp(subjectId, rows.calendar, rows.tasks,
      rows.metadata.flatMap((row) => row.task ? [row.task] : []), now),
    events: deriveEvents(subjectId, weeks, key, weeksData.classes, weeksData.lectures, now),
    yours: deriveYours(subjectId, weeks, key, files, live, rows.completed),
    behind: deriveBehind(subjectId, weeks, weeksData.lectures, weeksData.classes, now),
    progress: deriveProgress(subjectId, weeks, weeksData.lectures),
    revisionDue: subjectRevisionDue(rows.revision, subjectId, now.getTime()),
    sinceLastVisit, markVisited,
    loading: (loading && filesData.error == null) || weeksData.loading || (!threadsLoaded && threadError == null),
    error: filesData.error ?? error ?? weeksData.error ?? threadError,
  }), [subjectId, weeks, key, files, weeksData.lectures, weeksData.classes, rows, sinceLastVisit,
    now, live, markVisited, loading, weeksData.loading, error, weeksData.error, filesData.error, threadsLoaded, threadError]);
}

function useThreadLoad(loaded: boolean) {
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (loaded) return;
    let cancelled = false;
    useHarnessStore.getState().loadThreads().catch((error) => {
      if (!cancelled) setError(String(error));
    });
    return () => { cancelled = true; };
  }, [loaded]);
  return loaded ? null : error;
}
