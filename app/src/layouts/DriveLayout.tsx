import { Outlet, useOutletContext } from "react-router-dom";

import { DriveNav } from "@/components/drive/nav/DriveNav";
import { DRIVE_NAV_PANEL } from "@/components/drive/nav/panel";
import { SideNavShell } from "@/components/ui/layout/SideNavShell";
import { useSideNavPanel } from "@/hooks/ui/useSideNavPanel";

type Panel = ReturnType<typeof useSideNavPanel>;

/**
 * Everything under /drive: the nav column (My Drive, Recent, the folder
 * tree) beside the page. The column's width and fold live here so each
 * page's header can hold its toggle (`useDrivePanel`).
 */
export default function DriveLayout() {
  const panel = useSideNavPanel(DRIVE_NAV_PANEL);
  return (
    <SideNavShell panel={panel} nav={<DriveNav panel={panel} />}>
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <Outlet context={panel satisfies Panel} />
      </div>
    </SideNavShell>
  );
}

/** The nav column's panel, for a page's header toggle. */
export function useDrivePanel(): Panel {
  return useOutletContext<Panel>();
}
