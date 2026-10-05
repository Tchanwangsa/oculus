import { Navigate, useNavigate } from "react-router-dom";
import { BookOpen } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { SubjectNavSkeleton } from "@/components/subjects/SubjectNav";
import { SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjects } from "@/hooks/useSubjects";
import { lastSubjectId } from "@/layouts/SubjectLayout";

/**
 * `/subjects` has no page of its own: it opens the last subject a pane showed,
 * else the first current subject, else the first past one. Only with nothing
 * synced does it render, to point at Sync.
 */
export default function SubjectsRedirect() {
  const { subjects, current, past, loading } = useSubjects();
  const navigate = useNavigate();

  // The shape SubjectLayout loads with, so the hand-off doesn't flash.
  if (loading) {
    return (
      <div className="flex h-full overflow-hidden">
        <SubjectNavSkeleton />
      </div>
    );
  }

  const last = lastSubjectId();
  const target = subjects.find((s) => s.id === last) ?? current[0] ?? past[0];
  if (target) return <Navigate to={`/subjects/${target.id}`} replace />;

  return (
    <SubjectEmpty icon={<BookOpen size={24} className="text-muted-foreground/40" />} title="No subjects synced yet.">
      <Button
        variant="link"
        className="h-auto p-0 text-xs font-normal"
        onClick={() => navigate("/sync")}
      >
        Run a sync →
      </Button>
    </SubjectEmpty>
  );
}
