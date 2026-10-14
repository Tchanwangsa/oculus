import { DetailBody } from "@/components/projects/detail/DetailBody";
import type { DbProject, DbProjectTask } from "@/lib/planning/projects";

export function TaskBody({ task, project, onSave }: {
  task: DbProjectTask;
  project: DbProject | null;
  onSave: (body: string | null) => void;
}) {
  return <DetailBody text={task.body} subjectId={project?.subject_id ?? null} kind="task" onSave={onSave} />;
}
