import { useMemo, useRef } from "react";
import {
  Outlet,
  useLocation,
  useNavigate,
  useOutletContext,
} from "react-router-dom";

import { DropOverlay } from "@/components/ui/DropOverlay";
import { PillTabs } from "@/components/ui/PillTabs";
import { useSubject } from "@/layouts/SubjectLayout";
import { useFileDrop } from "@/hooks/useFileDrop";
import { useUploadImport, type UploadImport } from "@/hooks/useUploadImport";
import type { Subject } from "@/lib/db";

/** The Files tab's sub-tabs, in strip order; each is a child route in
 *  `app/src/routes.tsx`. */
const SUB_TABS = [
  { value: "downloads", label: "Downloads" },
  { value: "uploads", label: "Uploads" },
  { value: "documents", label: "Documents" },
] as const satisfies ReadonlyArray<{ value: string; label: string }>;

type SubTab = (typeof SUB_TABS)[number]["value"];

/**
 * What the Files tab hands its sub-pages through the outlet. A superset of the
 * subject, not a `{ subject }` record, because `useOutletContext` reads the
 * nearest Outlet: any other shape would make `useSubject()` silently return it
 * on every page under this tab.
 */
export interface FilesTab extends Subject {
  upload: UploadImport;
}

/** Only valid under `SubjectFilesPage`. */
export function useFilesTab(): FilesTab {
  return useOutletContext<FilesTab>();
}

/**
 * One subject tab for every file: Downloads, Uploads, Documents as routed
 * sub-tabs (so restore, crumbs and ⌘-click key off the path), drawn as
 * `PillTabs` under the subject's underline strip.
 *
 * The whole tab is the drop target: a Finder drop goes to Uploads whichever
 * sub-tab is showing, so the import state lives here and survives the switch.
 * `useFileDrop` hit-tests against visibility, so a background tab never
 * claims a drop.
 */
export default function SubjectFilesPage() {
  const subject = useSubject();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const rootRef = useRef<HTMLDivElement>(null);

  // Anything else is the index route's one-frame redirect to Downloads.
  const segment = pathname.split("/").pop();
  const active: SubTab = SUB_TABS.some((t) => t.value === segment)
    ? (segment as SubTab)
    : "downloads";

  const upload = useUploadImport(subject);

  const dropping = useFileDrop(rootRef, (paths) => {
    void upload.add(paths);
    if (active !== "uploads") navigate("uploads");
  });

  const context = useMemo<FilesTab>(
    () => ({ ...subject, upload }),
    [subject, upload],
  );

  return (
    <div ref={rootRef} className="relative flex h-full flex-col">
      {/* `-ml-2` puts the first label's text on the column's edge. */}
      <div className="mx-auto w-full max-w-5xl shrink-0 px-6 pt-4">
        <PillTabs
          className="-ml-2"
          tabs={SUB_TABS}
          value={active}
          onChange={(to) => navigate(to)}
        />
      </div>

      <div className="min-h-0 flex-1">
        <Outlet context={context} />
      </div>

      {/* An overlay, because the whole tab accepts the drop. */}
      <DropOverlay show={dropping} label={`Drop to add to ${subject.code}`} />
    </div>
  );
}
