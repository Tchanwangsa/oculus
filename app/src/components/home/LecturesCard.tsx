import { useCallback, useState } from "react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { lectureLabel } from "@/lib/calendar";
import { getRecentlyWatchedLectures, type Lecture } from "@/lib/db";
import { displayCode } from "@/lib/format";
import { LECTURES_CHANGED_EVENT, lecturePagePath, progressLabel } from "@/lib/lectures";
import { openBeside } from "@/lib/tabRouters";
import { useHomeSection } from "./useHomeSection";

/** Write-back only, not `LECTURE_PROGRESS_EVENT`: it fires every few seconds
 *  of playback. Module-level for a stable reference. */
const EVENTS = [LECTURES_CHANGED_EVENT];

const MAX_ROWS = 5;

/**
 * Lectures most recently watched and not finished, newest first, each with
 * the time left and a progress bar. Opens a lecture the way `RecentRow` does.
 */
export function LecturesCard() {
  const [lectures, setLectures] = useState<(Lecture & { subject_code: string })[] | null>(null);

  const reload = useCallback(() => {
    getRecentlyWatchedLectures(MAX_ROWS)
      .then(setLectures)
      .catch((e) => {
        console.error(e);
        setLectures([]);
      });
  }, []);

  useHomeSection(reload, EVENTS);

  return (
    <section className="overflow-hidden rounded-lg border border-border">
      <h2 className="px-3 pt-3 pb-1 text-[13px] font-semibold text-foreground">Lectures</h2>
      {lectures && lectures.length === 0 && (
        <p className="px-3 pt-1 pb-3 text-[12px] text-muted-foreground">Nothing in progress</p>
      )}
      <div className="divide-y divide-border-subtle">
        {lectures?.map((lecture) => {
          const path = lecturePagePath(lecture);
          const watched = lecture.duration_seconds > 0
            ? Math.min(1, lecture.progress_seconds / lecture.duration_seconds)
            : 0;
          return (
            <button
              key={lecture.id}
              type="button"
              className="flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-surface"
              data-tab-href={path}
              onClick={() => openBeside(path)}
            >
              <SubjectIcon code={lecture.subject_code} size={14} />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[12px] text-foreground">
                  {lectureLabel(lecture.title, lecture.subject_code)}
                </span>
                <span className="block truncate text-[11px] tabular-nums text-muted-foreground">
                  {displayCode(lecture.subject_code)} · {progressLabel(lecture).text}
                </span>
                <span className="mt-1.5 block h-[3px] overflow-hidden rounded-full bg-muted">
                  <span className="block h-full bg-brand" style={{ width: `${watched * 100}%` }} />
                </span>
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
