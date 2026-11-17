import { ClockCounterClockwise, Folder } from "@phosphor-icons/react";
import { useLocation } from "react-router-dom";

import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { SideNav, SideNavHeading, SideNavLink } from "@/components/ui/layout/SideNav";
import type { useSideNavPanel } from "@/hooks/ui/useSideNavPanel";
import { DRIVE_HREF, DRIVE_RECENT_HREF, driveFolderHref, parseDriveRoute } from "@/lib/drive/routes";
import { DriveTree } from "../tree/DriveTree";

/**
 * Drive's nav column: My Drive (every subject) and Recent, then the folder
 * tree, its subjects under Subjects and the rest under Past subjects. The
 * tree's rows navigate this pane, and take drops as the browser's folders do.
 */
export function DriveNav({ panel }: { panel: ReturnType<typeof useSideNavPanel> }) {
  const { pathname } = useLocation();
  const current = parseDriveRoute(pathname);
  const { collapsed } = panel;
  return (
    <>
      <SideNav
        aria-label="Drive"
        width={panel.width}
        collapsed={collapsed}
        animate={!panel.dragging}
        header={<SideNavHeading icon={Folder}>Drive</SideNavHeading>}
      >
        <div className="mb-2">
          <SideNavLink to={DRIVE_HREF} end icon={Folder} label="My Drive" />
          <SideNavLink to={DRIVE_RECENT_HREF} icon={ClockCounterClockwise} label="Recent" />
        </div>
        {/* The root is My Drive's row; the tree lights only a folder below it. */}
        <DriveTree current={current || null} folderHref={driveFolderHref} />
      </SideNav>
      {!collapsed && (
        <ResizeHandle
          onMouseDown={panel.onMouseDown}
          dragging={panel.dragging}
          label="Resize Drive sidebar"
          className="-mx-0.5"
        />
      )}
    </>
  );
}
