import type { CSSProperties } from "react";
import { Link } from "react-router-dom";
import { ProjectIcon } from "../icons/ProjectIcon";
import { ProjectIconPicker } from "../icons/ProjectIconPicker";
import { TagChip } from "../tags/TagChip";
import type { DbTaskTag } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";
import { projectHref } from "../nav/projectHref";
import { AgentMark, DueChip, SubtaskToggle, TaskSubject } from "../tasks/TaskMarks";
import { SubtaskProgressIndicator } from "../tasks/SubtaskProgressIndicator";
import { ProjectMenu } from "./ProjectMenu";
import { StampCell } from "./TaskCells";
import type { ProjectRowActions } from "./useProjectActions";
import type { ProjectBranch } from "./projectTableData";

/** A project occupies the same property columns as the task rows below it. */
export function ProjectIndexRow({ branch: {project, progress}, open, onToggle, cols, colsStyle, now, actions, tags }: {
  tags: DbTaskTag[];
  branch: ProjectBranch;
  open: boolean;
  onToggle: () => void;
  cols: string;
  colsStyle: CSSProperties;
  now: Date;
  actions: ProjectRowActions;
}) {
  return <div style={colsStyle} className={cn(cols, "group/project min-h-10 py-1.5 transition-colors hover:bg-surface/60")}>
    <div className="flex min-w-0 items-center gap-1.5">
      <SubtaskToggle expandable={progress.total > 0} expanded={open} onToggle={onToggle} label={`${open ? "Collapse" : "Expand"} tasks in ${project.name}`} />
      <ProjectIconPicker project={project}>
        <button type="button" aria-label={`Change icon for ${project.name}`} className="-mx-0.5 flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-md outline-none transition-colors hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50">
          <ProjectIcon project={project} size={15} />
        </button>
      </ProjectIconPicker>
      <Link to={projectHref(project)} className={cn("truncate text-[13px] font-medium hover:underline", project.status === "archived" ? "text-muted-foreground" : "text-foreground")}>
        {project.name}
      </Link>
      <AgentMark source={project.source} />
      {progress.total > 0 && <button type="button" aria-label={`${progress.done} of ${progress.total} tasks complete; ${open ? "collapse" : "expand"} ${project.name}`} aria-expanded={open} onClick={onToggle}
        className="shrink-0 cursor-pointer rounded-full outline-none hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50">
        <SubtaskProgressIndicator progress={progress} />
      </button>}
      <ProjectMenu project={project} onRename={name => actions.onRename(project, name)} onArchive={() => actions.onArchive(project)}
        onUnarchive={() => actions.onUnarchive(project)} onDelete={() => actions.onDelete(project)}
        className="ml-auto shrink-0 opacity-0 transition-opacity will-change-[opacity] group-hover/project:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100" />
    </div>
    <span className="flex min-w-0 items-center gap-1 overflow-hidden px-1">
      {project.tags.length ? <>{project.tags.slice(0, 2).map(tag => <TagChip key={tag} tag={tags.find(t => t.name.toLowerCase() === tag.toLowerCase()) ?? { id: 0, name: tag, color: "slate", position: 0 }} />)}
        {project.tags.length > 2 && <span className="shrink-0 text-[11px] text-muted-foreground">+{project.tags.length - 2}</span>}</> : <span className="text-[11px] text-muted-foreground/50">—</span>}
    </span>
    <span className="flex min-w-0 px-1"><TaskSubject code={project.subject_code} /></span>
    <span className="truncate px-1">{project.due_at ? <DueChip dueAt={project.due_at} /> : <span className="text-[11px] text-muted-foreground/50">—</span>}</span>
    <StampCell value={project.updated_at} now={now} />
  </div>;
}
