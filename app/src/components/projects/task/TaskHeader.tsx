import { useState } from "react";
import { ArrowLeft, DotsThree, TrashSimple } from "@phosphor-icons/react";
import { projectHref } from "@/components/projects/nav/projectHref";
import { PageHeader, PaneTrail, useInSidePanel } from "@/components/tabs/PaneHeader";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useTasksPanel } from "@/layouts/TasksLayout";
import { navigateActive } from "@/lib/shell/tabRouters";
import type { DbProject, DbProjectTask } from "@/lib/planning/projects";
import { Breadcrumb } from "@/components/ui/Breadcrumb";
import { taskBreadcrumbs } from "@/components/projects/nav/taskBreadcrumbs";
import { taskHref } from "@/components/projects/nav/taskHref";

/** The task path includes its parent immediately before a subtask. */
export function TaskHeader({
  task,
  project,
  parent,
  onDelete,
}: {
  task: DbProjectTask;
  /** `null` on an unfiled task. */
  project: DbProject | null;
  parent: DbProjectTask | null;
  onDelete: () => void;
}) {
  const inSide = useInSidePanel();
  const panel = useTasksPanel();
  const back = parent ? taskHref(task.project_id, parent) : project ? projectHref(project) : "/tasks";
  const items = taskBreadcrumbs(task, project, parent);

  return (
    <PageHeader>
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
        <Breadcrumb items={items} />
      </PaneTrail>
      {/* The side panel's own controls follow this in the row. */}
      <TaskMenu onDelete={onDelete} />
    </PageHeader>
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
