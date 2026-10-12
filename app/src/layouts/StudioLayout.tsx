import { useEffect } from "react";
import { Outlet, useOutletContext } from "react-router-dom";

import { StudioNav } from "@/components/studio/nav/StudioNav";
import { STUDIO_NAV_PANEL } from "@/components/studio/nav/panel";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useVideosStore } from "@/stores/studio/videosStore";

type Panel = ReturnType<typeof useResizablePanel>;

/**
 * Everything under /studio: the nav column beside the page. The column's width
 * and fold live here so each page's header row can hold its toggle
 * (`useStudioPanel`). The video list is read once here, for the Recent rows
 * and the grid.
 */
export default function StudioLayout() {
  const panel = useResizablePanel(STUDIO_NAV_PANEL);
  useEffect(() => {
    void useVideosStore.getState().reload();
  }, []);

  return (
    <div className="flex h-full overflow-hidden">
      <StudioNav panel={panel} />
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <Outlet context={panel satisfies Panel} />
      </div>
    </div>
  );
}

/** The nav column's panel, for a page's header toggle. */
export function useStudioPanel(): Panel {
  return useOutletContext<Panel>();
}
