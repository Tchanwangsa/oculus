import { useState } from "react";
import { ArrowLeft, CaretRight, DotsThree, TrashSimple } from "@phosphor-icons/react";
import { projectHref } from "@/components/projects/nav/projectHref";
import { taskVariant } from "@/components/projects/tasks/columnVariant";
import { TaskGlyph } from "@/components/projects/tasks/TaskMarks";
import { PaneHeaderRow, PaneTrail, useInSidePanel } from "@/components/tabs/PaneHeader";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useTasksPanel } from "@/layouts/TasksLayout";
import { navigateActive } from "@/lib/shell/tabRouters";
import type { DbProject, DbProjectTask } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";

/**
 * The task page's header row: the nav column's toggle, a way back, the trail
 * `<project> › <glyph> title` (`Tasks` while unfiled; a subtask shows its
 * parent's project), and the task's menu. The first crumb links to the
 * project, or to the Tasks list. In the side panel the row is the panel's
 * header (`PaneHeaderRow`) and the trail scrolls sideways.
 */
export function TaskHeader({
  task,
  project,
  onDelete,
}: {
  task: DbProjectTask;
  /** `null` on an unfiled task. */
  project: DbProject | null;
  onDelete: () => void;
}) {
  const inSide = useInSidePanel();
  const panel = useTasksPanel();
  const back = project ? projectHref(project) : "/tasks";

  return (
    <PaneHeaderRow
      className={cn(
        "flex shrink-0 items-center gap-2.5 border-b border-border-subtle",
        inSide ? "h-11 px-5" : "h-12 px-6",
      )}
    >
      {panel && <SideNavCollapseToggle collapsed={panel.collapsed} onToggle={panel.toggle} />}
      {!inSide && (
        <button
          type="button"
          aria-label="Back"
          data-tab-href={back}
          onClick={() => navigateActive(back)}
          className="-mr-1 flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <ArrowLeft size={15} />
        </button>
      )}
      <PaneTrail>
        <nav
          aria-label="Breadcrumb"
          className="flex min-w-0 flex-1 items-center gap-1.5 text-[13px] text-muted-foreground"
        >
          <button
            type="button"
            data-tab-href={back}
            onClick={() => navigateActive(back)}
            className={cn(
              "cursor-pointer transition-colors hover:text-foreground",
              inSide ? "shrink-0 whitespace-nowrap" : "max-w-[40%] min-w-0 shrink truncate",
            )}
          >
            {project?.name ?? "Tasks"}
          </button>
          <CaretRight size={10} className="shrink-0" />
          <TaskGlyph variant={taskVariant(project, task.column_id)} size={14} />
          <span
            className={cn(
              "font-medium text-foreground",
              inSide ? "shrink-0 whitespace-nowrap" : "min-w-0 truncate",
            )}
          >
            {task.title}
          </span>
        </nav>
      </PaneTrail>
      {/* The side panel's own controls follow this in the row. */}
      <TaskMenu onDelete={onDelete} />
    </PaneHeaderRow>
  );
}

/** The `…` menu. Delete only asks: the page's dialog confirms. */
function TaskMenu({ onDelete }: { onDelete: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Task menu"
          className="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground data-[state=open]:bg-accent"
        >
          <DotsThree size={18} weight="bold" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-40 p-1">
        <button
          type="button"
          onClick={() => {
            setOpen(false);
            onDelete();
          }}
          className="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-destructive transition-colors hover:bg-destructive/10"
        >
          <TrashSimple size={13} className="shrink-0" />
          Delete task
        </button>
      </PopoverContent>
    </Popover>
  );
}
