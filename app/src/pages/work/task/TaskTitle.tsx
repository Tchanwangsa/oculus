import { DetailTitle } from "@/components/projects/detail/DetailTitle";
import { taskHref } from "@/components/projects/nav/taskHref";
import type { DbProjectTask } from "@/lib/planning/projects";

export function TaskTitle({ task, onRename }: { task: DbProjectTask; onRename: (title: string) => void }) {
  return <DetailTitle title={task.title} label="Task title" done={task.done_at != null}
    href={(title) => taskHref(task.project_id, { id: task.id, title })} onRename={onRename} />;
}
