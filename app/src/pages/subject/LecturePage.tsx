import { useCallback, useEffect, useState } from "react";
import { Navigate, useParams, useSearchParams } from "react-router-dom";
import { CircleNotch } from "@phosphor-icons/react";
import { getLectures, type Lecture } from "@/lib/db";
import { LecturePlayer } from "@/components/lectures/LecturePlayer";
import { LECTURES_TAB, SubjectCrumbs } from "@/components/subjects/SubjectCrumbs";
import { recordRecent } from "@/lib/recents";

/**
 * A lecture as its own page — what the peek's expand button opens in a new
 * tab. Standalone: no subject shell above it, so the player has the height,
 * and one breadcrumb row says which subject this recording belongs to and
 * leads back to its Lectures list. Fullscreen takes the player `fixed
 * inset-0`, which covers the row rather than fighting it.
 */
export default function SubjectLecturePage() {
  const { subjectId } = useParams();
  const [searchParams] = useSearchParams();
  const lectureId = searchParams.get("id");
  const id = Number(subjectId);

  const [lecture, setLecture] = useState<Lecture | null>(null);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    if (!Number.isFinite(id) || !lectureId) return;
    const rows = await getLectures(id);
    setLecture(rows.find((l) => l.id === lectureId) ?? null);
    setLoaded(true);
  }, [id, lectureId]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // Feeds the "Recently visited" row on the subject home.
  useEffect(() => {
    if (lecture) {
      recordRecent(lecture.subject_id, {
        kind: "lecture",
        ref: lecture.id,
        title: lecture.title,
      });
    }
  }, [lecture?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!Number.isFinite(id) || !lectureId) return <Navigate to="/subjects" replace />;

  if (!lecture) {
    if (!loaded) {
      return (
        <div className="h-full flex items-center justify-center gap-2 text-muted-foreground">
          <CircleNotch size={16} className="animate-spin" />
          <span className="text-sm">Loading…</span>
        </div>
      );
    }
    // Loaded but gone — a stale tab after a re-sync.
    return <Navigate to={`/subjects/${id}/lectures`} replace />;
  }

  return (
    <div className="h-full flex flex-col overflow-hidden bg-background">
      <div className="h-11 shrink-0 flex items-center gap-2.5 px-5 border-b border-border-subtle">
        <nav
          aria-label="Breadcrumb"
          className="flex shrink-0 items-center gap-2.5 text-[11px] text-muted-foreground"
        >
          <SubjectCrumbs subjectId={id} tab={LECTURES_TAB} />
        </nav>
        <h1 className="flex-1 min-w-0 text-[13px] font-semibold text-foreground truncate">
          {lecture.title}
        </h1>
      </div>
      <LecturePlayer lecture={lecture} onRefresh={refresh} />
    </div>
  );
}
