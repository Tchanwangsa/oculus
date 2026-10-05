import { useEffect, useMemo } from "react";
import { Navigate, Outlet, useOutletContext, useParams } from "react-router-dom";
import { useSubjects } from "@/hooks/useSubjects";
import { SubjectNav, SubjectNavSkeleton } from "@/components/subjects/SubjectNav";
import type { Subject } from "@/lib/db";

/**
 * Everything under /subjects/:subjectId: the subject's nav column beside the
 * tab. Resolves the subject once; child pages read it via `useSubject()`.
 * Each tab owns the full remaining height.
 */
export default function SubjectLayout() {
  const { subjectId } = useParams();
  const id = Number(subjectId);
  const { subjects, current, past, loading } = useSubjects();

  const subject = useMemo(
    () => subjects.find((s) => s.id === id) ?? null,
    [subjects, id],
  );

  useEffect(() => {
    if (subject) document.title = `${subject.code} · Oculus`;
    return () => {
      document.title = "Oculus";
    };
  }, [subject]);

  if (!Number.isFinite(id)) return <Navigate to="/subjects" replace />;

  if (loading && !subject) {
    return (
      <div className="flex h-full overflow-hidden">
        <SubjectNavSkeleton />
      </div>
    );
  }

  // Loaded but no such subject — a stale id, e.g. after clearing the DB.
  if (!subject) return <Navigate to="/subjects" replace />;

  return (
    <div className="flex h-full overflow-hidden">
      <SubjectNav subject={subject} current={current} past={past} />

      {/* Keyed so switching subjects remounts the tab. */}
      <div key={subject.id} className="min-w-0 flex-1 overflow-hidden">
        <Outlet context={subject satisfies Subject} />
      </div>
    </div>
  );
}

/** The subject for the current /subjects/:subjectId route. */
export function useSubject(): Subject {
  return useOutletContext<Subject>();
}
