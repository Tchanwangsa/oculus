import { useCallback, useEffect, useState } from "react";
import { Navigate, useParams, useSearchParams } from "react-router-dom";
import { getLectures, type Lecture } from "@/lib/db";
import { LecturePlayer } from "@/components/lectures/LecturePlayer";
import { LoadingFill } from "@/components/ui/PageParts";
import { LECTURES_TAB, SubjectCrumbs } from "@/components/subjects/SubjectCrumbs";
import { recordRecent } from "@/lib/recents";

/**
 * A lecture as its own page (the peek's expand target). Standalone, so the
 * player has the height; one breadcrumb row leads back to the subject.
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
    if (!loaded) return <LoadingFill />;
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
