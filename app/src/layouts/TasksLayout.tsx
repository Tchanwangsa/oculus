import { Outlet, useOutletContext } from "react-router-dom";

import { TASKS_NAV_PANEL } from "@/components/projects/nav/panel";
import { TasksNav } from "@/components/projects/nav/TasksNav";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";

type Panel = ReturnType<typeof useResizablePanel>;

/**
 * Everything under Tasks and Projects: the nav column beside the page. The
 * column's width and fold live here so each page's header row can hold its
 * toggle (`useTasksPanel`).
 */
export default function TasksLayout() {
  const panel = useResizablePanel(TASKS_NAV_PANEL);

  return (
    <div className="flex h-full overflow-hidden">
      <TasksNav panel={panel} />
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <Outlet context={panel satisfies Panel} />
      </div>
    </div>
  );
}

/** The nav column's panel, for a page's header toggle. `null` where the page
 *  renders outside the layout (a task opened in the side panel). */
export function useTasksPanel(): Panel | null {
  return useOutletContext<Panel | null>() ?? null;
}
