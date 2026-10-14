import { useMemo } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { CalendarCheck, CalendarX, Kanban, ListChecks, Sparkle, type Icon } from "@phosphor-icons/react";
import { NewTaskButton } from "@/components/projects/nav/NewTaskButton";
import { TagsGroup } from "@/components/projects/nav/TagsGroup";
import { TasksNavRow } from "@/components/projects/nav/TasksNavRow";
import { useCreateTask } from "@/components/projects/nav/useCreateTask";
import { parseTagParam, parseView, tasksHref, TASK_VIEWS, type TaskViewId } from "@/components/projects/tasks/taskViews";
import { useTabActive } from "@/components/tabs/TabContext";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { SIDE_NAV_FOLDS, SideNav } from "@/components/ui/layout/SideNav";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { useTaskList } from "@/hooks/data/useTaskList";
import { useTaskTags } from "@/hooks/data/useTaskTags";
import type { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { cn } from "@/lib/utils";

const VIEW_ICONS: Record<TaskViewId, Icon> = {
  all: ListChecks,
  week: CalendarCheck,
  undated: CalendarX,
  agent: Sparkle,
};

/**
 * The Tasks section's nav column: New task, the views, Projects and the tag
 * registry. A view and a tag are URL state on `/tasks` and compose, each link
 * keeping the other, so ⌘-click and tab restore carry both. ⌘⌥B folds it.
 */
export function TasksNav({ panel }: { panel: ReturnType<typeof useResizablePanel> }) {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const { tasks, projects } = useTaskList("all");
  const { tags } = useTaskTags();
  const create = useCreateTask();
  const { collapsed } = panel;

  const onTasks = pathname === "/tasks";
  const params = new URLSearchParams(search);
  // Off the Tasks page neither is selected, and a link starts clean.
  const view = onTasks ? parseView(params.get("view")) : null;
  const tag = onTasks ? parseTagParam(params.get("tag")) : null;

  const counts = useMemo(() => {
    const byTag = new Map<number, number>();
    for (const t of tasks) for (const id of t.tag_ids) byTag.set(id, (byTag.get(id) ?? 0) + 1);
    return byTag;
  }, [tasks]);

  // Every tab stays mounted, so only the one in front answers the chord.
  // `e.code`, not `e.key`: on macOS ⌥B arrives as `∫`.
  const active = useTabActive();
  useWindowEvent("keydown", (e) => {
    const k = e as KeyboardEvent;
    if (!active || !k.altKey || !(k.metaKey || k.ctrlKey) || k.code !== "KeyB") return;
    k.preventDefault();
    panel.toggle();
  });

  return (
    <>
      <SideNav
        aria-label="Tasks"
        width={panel.width}
        collapsed={collapsed}
        animate={!panel.dragging}
        header={
          <>
            <div className="px-4 pt-4 pb-3">
              <h1
                className={cn(
                  "font-display text-[16px] font-semibold leading-none tracking-tight text-foreground",
                  SIDE_NAV_FOLDS,
                )}
              >
                Tasks
              </h1>
            </div>
            <div className={cn("px-3 pb-2", SIDE_NAV_FOLDS)}>
              <NewTaskButton projects={projects} onCreate={create} align="start" className="w-full" />
            </div>
          </>
        }
      >
        <div className="flex flex-col gap-0.5">
          {TASK_VIEWS.map((v) => (
            <TasksNavRow
              key={v.id}
              to={tasksHref(v.id, tag)}
              active={view === v.id}
              icon={VIEW_ICONS[v.id]}
              label={v.label}
            />
          ))}
          <TasksNavRow
            to="/projects"
            active={pathname === "/projects" || pathname.startsWith("/projects/")}
            icon={Kanban}
            label="Projects"
          />
        </div>

        <TagsGroup
          tags={tags}
          counts={counts}
          view={view ?? "all"}
          selected={tag}
          onDeleted={(id) => {
            if (id === tag && view) navigate(tasksHref(view, null), { replace: true });
          }}
        />
      </SideNav>
      {/* On the seam: negative margins cost no layout width. */}
      {!collapsed && (
        <ResizeHandle
          onMouseDown={panel.onMouseDown}
          dragging={panel.dragging}
          label="Resize Tasks sidebar"
          className="-mx-0.5"
        />
      )}
    </>
  );
}
