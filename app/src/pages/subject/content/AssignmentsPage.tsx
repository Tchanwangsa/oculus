import { useMemo } from "react";
import { SubjectLoading, SubjectPage } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useSubject } from "@/layouts/SubjectLayout";
import { useCourseFileData } from "@/hooks/data/useCourseFileData";
import { ListCard } from "@/components/ui/layout/PageParts";
import { GROUPS, GROUP_LABELS, groupOf, parseTaskDoc, type Group, type TaskDoc } from "./assignments/grouping";
import { TaskRow } from "./assignments/TaskRow";
import { TocFallback } from "./assignments/TocFallback";

/**
 * Quizzes and assignments from `assignments/` and `quizzes/`, due soonest
 * first. With no such documents yet, falls back to the module-TOC listing,
 * whose rows can only link out to Canvas.
 */
export default function SubjectAssignmentsPage() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);

  const taskFiles = useMemo(
    () => [...byCategory.assignment, ...byCategory.quiz],
    [byCategory.assignment, byCategory.quiz],
  );
  const docs = useCourseFileData(taskFiles, loading, parseTaskDoc);

  const grouped = useMemo(() => {
    if (!docs) return null;
    // Dated tasks by deadline; the undated tail keeps a stable name order.
    const sorted = [...docs].sort((a, b) => {
      if (a.due && b.due) return a.due.getTime() - b.due.getTime();
      if (a.due !== b.due) return a.due ? -1 : 1;
      return a.title.localeCompare(b.title);
    });
    const now = Date.now();
    const byGroup = new Map<Group, TaskDoc[]>();
    for (const t of sorted) {
      const g = groupOf(t, now);
      byGroup.set(g, [...(byGroup.get(g) ?? []), t]);
    }
    return byGroup;
  }, [docs]);

  if (grouped == null) {
    return <SubjectLoading count={4} rowClassName="h-10" />;
  }

  if ([...grouped.values()].every((g) => g.length === 0)) {
    return <TocFallback />;
  }

  return (
    <SubjectPage className="py-5 space-y-5">
      {GROUPS.map((group) => {
        const tasks = grouped.get(group);
        if (!tasks || tasks.length === 0) return null;
        return (
          <section key={group}>
            <h2 className="mb-2 px-0.5 text-[13px] font-semibold text-foreground">
              {GROUP_LABELS[group]}
            </h2>
            <ListCard>
              {tasks.map((t) => (
                <TaskRow key={t.file.id} task={t} muted={group === "closed" || group === "done"} />
              ))}
            </ListCard>
          </section>
        );
      })}
    </SubjectPage>
  );
}
