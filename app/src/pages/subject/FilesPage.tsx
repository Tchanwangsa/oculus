import { useMemo, useRef } from "react";
import {
  Outlet,
  useLocation,
  useNavigate,
  useOutletContext,
} from "react-router-dom";

import { cn } from "@/lib/utils";
import { PillTabs } from "@/components/ui/PillTabs";
import { useSubject } from "@/layouts/SubjectLayout";
import { useFileDrop } from "@/hooks/useFileDrop";
import { useUploadImport, type UploadImport } from "@/hooks/useUploadImport";
import type { Subject } from "@/lib/db";

/**
 * The Files tab's sub-tabs, keyed by the path segment under `files/`. The
 * order is the strip's order.
 *
 * Each has a matching child route in `app/src/routes.tsx`.
 */
const SUB_TABS = [
  { value: "downloads", label: "Downloads" },
  { value: "uploads", label: "Uploads" },
  { value: "documents", label: "Documents" },
] as const satisfies ReadonlyArray<{ value: string; label: string }>;

type SubTab = (typeof SUB_TABS)[number]["value"];

/**
 * What the Files tab hands its sub-pages through the outlet.
 *
 * A **superset of the subject**, not a record with a `subject` field, and
 * that is load-bearing: `useOutletContext` reads the *nearest* Outlet's
 * context, and this page's Outlet is nearer to the sub-pages than
 * `SubjectLayout`'s. A context of its own shape would have made `useSubject()`
 * answer with it, wrongly and without a type error, on every page under this
 * tab. Spreading the subject in keeps `useSubject()` true here, so a sub-page
 * written like every other subject tab still works; `useFilesTab()` is the
 * same object with the upload half typed.
 */
export interface FilesTab extends Subject {
  upload: UploadImport;
}

/** The Files tab's context: the subject, and the upload import state that the
 *  whole tab shares. Only valid under `SubjectFilesPage`. */
export function useFilesTab(): FilesTab {
  return useOutletContext<FilesTab>();
}

/**
 * One subject tab for everything that is a file: what Canvas gave you
 * (Downloads) and what you added yourself (Uploads), as sub-tabs of one strip.
 *
 * **The strip is routed, not local**, for the reason `SectionHeader` gives:
 * each sub-tab is a path (`files/downloads`, `files/uploads`), so a restored
 * tab, a crumb, ⌘-click and `tabInfo` all key off it with nothing new to
 * learn. It is a `PillTabs` rather than a second underline strip — the
 * subject's own tabs are the underline above it, and two stacked underline
 * strips read as two levels of the same rank.
 *
 * **The whole tab is the drop target.** A file dragged in from Finder lands
 * on Uploads whichever sub-tab is showing, because that is the only place a
 * dropped file can go, and asking the student to switch to Uploads first
 * before the drag would take is a step nobody discovers. The import state
 * (`useUploadImport`) therefore lives here rather than in the Uploads page,
 * so the "Adding…" rows survive the switch to it; the sub-pages reach it
 * through `useFilesTab()`.
 *
 * The drop itself is `useFileDrop` on this page's root — the hook the
 * composers use, which listens as the webview, measures the point→pixel
 * ratio itself and hit-tests the drop against the element. Hit-testing is
 * what excludes a background tab: panes are hidden with `visibility`, which
 * the hook checks, so a drop meant for the tab in front is never claimed by a
 * Files tab behind it and no `useTabActive` gate is needed.
 */
export default function SubjectFilesPage() {
  const subject = useSubject();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const rootRef = useRef<HTMLDivElement>(null);

  // The last segment is the sub-tab; the index route redirects to Downloads,
  // so a pathname that names neither is that redirect's one frame.
  const segment = pathname.split("/").pop();
  const active: SubTab = SUB_TABS.some((t) => t.value === segment)
    ? (segment as SubTab)
    : "downloads";

  const upload = useUploadImport(subject);

  const dropping = useFileDrop(rootRef, (paths) => {
    void upload.add(paths);
    // The rows land in Uploads; show them landing.
    if (active !== "uploads") navigate("uploads");
  });

  const context = useMemo<FilesTab>(
    () => ({ ...subject, upload }),
    [subject, upload],
  );

  return (
    <div ref={rootRef} className="relative flex h-full flex-col">
      {/* Same centred column as the pages below; `-ml-2` puts the first
          label's text, not its pill, on the column's edge. */}
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

      {/* The drop affordance is an overlay rather than a bordered zone in the
          layout: the whole tab accepts files, and a zone that only covers part
          of it would be a lie about where a drop lands. */}
      <div
        aria-hidden
        className={cn(
          "pointer-events-none absolute inset-3 flex items-center justify-center rounded-xl border-2 border-dashed border-brand bg-brand/5 transition-opacity duration-150",
          dropping ? "opacity-100" : "opacity-0",
        )}
      >
        <span className="rounded-full bg-card px-3 py-1.5 text-[13px] font-medium text-brand shadow-sm">
          Drop to add to {subject.code}
        </span>
      </div>
    </div>
  );
}
