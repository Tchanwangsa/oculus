import { ArrowElbowDownRight } from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode } from "@/lib/format/format";
import type { DbProject, DbTaskTag, DbTaskWithProject } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";
import type { PropertyId } from "../display/settings";
import { StampCell } from "../lists/TaskCells";
import { taskHref } from "../nav/taskHref";
import { TagList } from "../tags/TagList";
import { AgentMark, DueChip, TaskGlyph } from "../tasks/TaskMarks";
import { SubtaskProgressIndicator } from "../tasks/SubtaskProgressIndicator";
import { taskVariant } from "../tasks/columnVariant";
import type { SubtaskProgress } from "../tasks/taskTree";
import { projectLabel, projectOf } from "../tasks/universalTasks";
import { CardTitle } from "./CardTitle";

/** One card body for every task board, including the lifted drag preview. */
export function TaskCard({ task, parent, progress, projectById, tagById, properties, now }: {
  task: DbTaskWithProject;
  parent: DbTaskWithProject | null;
  progress?: SubtaskProgress;
  projectById: Map<number, DbProject>;
  tagById: Map<number, DbTaskTag>;
  properties: readonly PropertyId[];
  now: Date;
}) {
  const subject = properties.includes("subject") ? task.project_subject_code : null;
  const project = properties.includes("project");
  const context = [subject ? displayCode(subject) : null, project ? projectLabel(task) : null].filter(Boolean).join(" · ");
  return <>
    {context && <div className="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
      {subject && <SubjectIcon code={subject} size={11} />}
      <span className="truncate">{context}</span>
    </div>}
    {parent && <div className="mt-0.5 flex min-w-0 items-center gap-1 text-[11px] text-muted-foreground/60">
      <ArrowElbowDownRight size={10} className="shrink-0" />
      <span className="truncate">{parent.title}</span>
    </div>}
    <div className={cn("flex min-w-0 items-start gap-1.5", (context || parent) && "mt-1")}>
      <TaskGlyph variant={taskVariant(projectOf(task, projectById), task.column_id)} size={13} className="mt-0.5" />
      <CardTitle title={task.title} href={taskHref(task.project_id, task)} done={task.done_at != null} />
      <AgentMark source={task.source} className="mt-0.5" />
    </div>
    {properties.map((property) => {
      switch (property) {
        case "tags": return task.tag_ids.length > 0 && <div key={property} className="mt-1.5 flex min-w-0 pl-[18px]"><TagList tagIds={task.tag_ids} byId={tagById} max={3} /></div>;
        case "due": return task.due_at && <div key={property} className="mt-1.5 pl-[18px]"><DueChip dueAt={task.due_at} /></div>;
        case "updated":
        case "created": return <div key={property} className="mt-1.5 flex items-center gap-1 pl-[18px] text-[11px] text-muted-foreground"><span>{property === "updated" ? "Updated" : "Created"}</span><StampCell value={property === "updated" ? task.updated_at : task.created_at} now={now} /></div>;
        default: return null;
      }
    })}
    {progress != null && progress.total > 0 && <div className="mt-1.5 pl-[18px]"><SubtaskProgressIndicator progress={progress} /></div>}
  </>;
}
