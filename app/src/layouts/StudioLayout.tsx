import { useEffect } from "react";
import { Outlet, useOutletContext } from "react-router-dom";

import { StudioNav } from "@/components/studio/nav/StudioNav";
import { STUDIO_NAV_PANEL } from "@/components/studio/nav/panel";
import { useSideNavPanel } from "@/hooks/ui/useSideNavPanel";
import { SideNavShell } from "@/components/ui/layout/SideNavShell";
import { useVideosStore } from "@/stores/studio/videosStore";

type Panel = ReturnType<typeof useSideNavPanel>;

/**
 * Everything under /studio: the nav column beside the page. The column's width
 * and fold live here so each page's header row can hold its toggle
 * (`useStudioPanel`). The video list is read once here, for the Recent rows
 * and the grid.
 */
export default function StudioLayout() {
  const panel = useSideNavPanel(STUDIO_NAV_PANEL);
  useEffect(() => {
    void useVideosStore.getState().reload();
  }, []);

  return (
    <SideNavShell panel={panel} nav={<StudioNav panel={panel} />}>
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <Outlet context={panel satisfies Panel} />
      </div>
    </SideNavShell>
  );
}

/** The nav column's panel, for a page's header toggle. */
export function useStudioPanel(): Panel {
  return useOutletContext<Panel>();
}
