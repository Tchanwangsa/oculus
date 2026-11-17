import { useEffect, useMemo } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import { DriveBrowser } from "@/components/drive";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useSubject, useSubjectPanel } from "@/layouts/SubjectLayout";
import { resolveLegacyTab } from "@/lib/drive/legacyTabs";
import { listFolder } from "@/lib/drive/listing";
import { filesFolderHref, parseFilesRoute } from "@/lib/drive/routes";
import { subjectRel } from "@/lib/files/libraryLayout";

/**
 * A subject's Files tab: Drive's folder browser rooted at the subject's
 * folder, the route's path below `files/` the folder on screen
 * (`app/src/lib/drive/routes.ts`). An old sub-tab's path (`files/downloads`
 * and kin, from a saved tab or Recent) is replaced by the folder its files
 * are in now, unless the student has a folder by that name.
 */
export default function SubjectFilesPage() {
  const subject = useSubject();
  const panel = useSubjectPanel();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const root = subjectRel(subject.code);
  const inner = parseFilesRoute(pathname)?.inner ?? "";
  const folder = inner ? `${root}/${inner}` : root;

  const folderHref = useMemo(
    () => (rel: string) => filesFolderHref(subject.id, rel === root ? "" : rel.slice(root.length + 1)),
    [subject.id, root],
  );

  useEffect(() => {
    let live = true;
    const exists = (rel: string) =>
      listFolder(rel ? `${root}/${rel}` : root).then(() => true, () => false);
    void resolveLegacyTab(inner, exists).then((target) => {
      if (live && target != null) navigate(filesFolderHref(subject.id, target), { replace: true });
    });
    return () => {
      live = false;
    };
  }, [inner, root, subject.id, navigate]);

  return (
    <DriveBrowser
      root={root}
      folder={folder}
      folderHref={folderHref}
      rootLabel="Files"
      leading={<SideNavCollapseToggle panel={panel} />}
    />
  );
}
