import { useCallback, useEffect, useState } from "react";
import { Navigate, useParams, useSearchParams } from "react-router-dom";
import { CircleNotch } from "@phosphor-icons/react";
import { getLectures, type Lecture } from "@/lib/db";
import { LecturePlayer } from "@/components/lectures/LecturePlayer";
import { recordRecent } from "@/lib/recents";

/**
 * A lecture as its own page — what the peek's expand button opens in a new
 * tab. Fully standalone: no subject shell, just the player edge to edge.
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
      <LecturePlayer lecture={lecture} onRefresh={refresh} />
    </div>
  );
}
