import { useLocation } from "react-router-dom";

import { DriveBrowser } from "@/components/drive";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useDrivePanel } from "@/layouts/DriveLayout";
import { driveFolderHref, parseDriveRoute } from "@/lib/drive/routes";

/** One folder of Drive, every subject's tree from the top: the route's path
 *  is the folder (`/drive/<code>/…`, `app/src/lib/drive/routes.ts`). */
export default function DriveFolderPage() {
  const panel = useDrivePanel();
  const { pathname } = useLocation();
  const folder = parseDriveRoute(pathname) ?? "";
  return (
    <DriveBrowser
      root=""
      folder={folder}
      folderHref={driveFolderHref}
      leading={<SideNavCollapseToggle panel={panel} />}
    />
  );
}
