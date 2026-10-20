import { Outlet, useOutletContext } from "react-router-dom";

import { TASKS_NAV_PANEL } from "@/components/projects/nav/panel";
import { TasksNav } from "@/components/projects/nav/TasksNav";
import { useSideNavPanel } from "@/hooks/ui/useSideNavPanel";
import { SideNavShell } from "@/components/ui/layout/SideNavShell";

type Panel = ReturnType<typeof useSideNavPanel>;

/**
 * Everything under Tasks and Projects: the nav column beside the page. The
 * column's width and fold live here so each page's header row can hold its
 * toggle (`useTasksPanel`).
 */
export default function TasksLayout() {
  const panel = useSideNavPanel(TASKS_NAV_PANEL);

  return (
    <SideNavShell panel={panel} nav={<TasksNav panel={panel} />}>
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <Outlet context={panel satisfies Panel} />
      </div>
    </SideNavShell>
  );
}

/** The nav column's panel, for a page's header toggle. `null` where the page
 *  renders outside the layout (a task opened in the side panel). */
export function useTasksPanel(): Panel | null {
  return useOutletContext<Panel | null>() ?? null;
}
