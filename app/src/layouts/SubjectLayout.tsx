import { createContext, useContext, useEffect, useMemo } from "react";
import { Navigate, Outlet, useLocation, useOutletContext, useParams } from "react-router-dom";
import { useSideNavPanel } from "@/hooks/ui/useSideNavPanel";
import { SideNavShell } from "@/components/ui/layout/SideNavShell";
import { useSubjects } from "@/hooks/data/useSubjects";
import { SubjectCrumbs } from "@/components/subjects/SubjectCrumbs";
import {
  SUBJECT_NAV_PANEL,
  SubjectNav,
  SubjectNavSkeleton,
  subjectTabLabel,
} from "@/components/subjects/SubjectNav";
import { PaneHeaderRow, PaneTitle, PaneTrail } from "@/components/tabs/PaneHeader";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import type { Subject } from "@/lib/db";

const LAST_SUBJECT_KEY = "oculus-last-subject";

type Panel = ReturnType<typeof useSideNavPanel>;

/** The nav column's panel, for a tab that draws its own header row. A
 *  context, not the outlet's: that carries the subject (`useSubject`). */
const SubjectPanelContext = createContext<Panel | null>(null);

/** Tabs whose page draws the header row itself (Files: Drive's browser,
 *  whose trail goes on from the subject). */
const OWN_HEADER_TABS = new Set(["files"]);

/** The subject a page last resolved, for `/subjects` to reopen
 *  (`app/src/pages/start/SubjectsRedirect.tsx`). The caller checks it still exists. */
export function lastSubjectId(): number | null {
  try {
    const id = Number(localStorage.getItem(LAST_SUBJECT_KEY));
    return Number.isInteger(id) && id > 0 ? id : null;
  } catch {
    return null;
  }
}

function rememberSubject(id: number): void {
  try {
    localStorage.setItem(LAST_SUBJECT_KEY, String(id));
  } catch {
    /* quota or private mode — `/subjects` falls back to the first subject */
  }
}

/**
 * Everything under /subjects/:subjectId: the subject's nav column beside the
 * tab, under a header row shaped like Chat's — the column's toggle, then the
 * subject and the tab's name. Resolves the subject once; child pages read it
 * via `useSubject()`. Each tab owns the full height under the header.
 */
export default function SubjectLayout() {
  const { subjectId } = useParams();
  const id = Number(subjectId);
  const { subjects, current, past, loading } = useSubjects();
  const panel = useSideNavPanel(SUBJECT_NAV_PANEL);
  const { pathname } = useLocation();

  const subject = useMemo(
    () => subjects.find((s) => s.id === id) ?? null,
    [subjects, id],
  );

  const resolvedId = subject?.id;
  useEffect(() => {
    if (resolvedId != null) rememberSubject(resolvedId);
  }, [resolvedId]);

  useEffect(() => {
    if (subject) document.title = `${subject.code} · Oculus`;
    return () => {
      document.title = "Oculus";
    };
  }, [subject]);

  if (!Number.isFinite(id)) return <Navigate to="/subjects" replace />;

  if (loading && !subject) {
    return (
      <SideNavShell panel={panel} nav={<SubjectNavSkeleton panel={panel} />}><div /></SideNavShell>
    );
  }

  // Loaded but no such subject — a stale id, e.g. after clearing the DB. The
  // redirect skips a remembered id that is gone, so it never bounces back.
  if (!subject) return <Navigate to="/subjects" replace />;

  const ownHeader = OWN_HEADER_TABS.has(pathname.split("/")[3] ?? "");
  return (
    <SideNavShell panel={panel} nav={<SubjectNav subject={subject} current={current} past={past} panel={panel} />}>

      <div className="flex h-full min-w-0 flex-1 flex-col">
        {!ownHeader && <PaneHeaderRow className="flex h-12 shrink-0 items-center gap-2.5 border-b border-border-subtle px-6">
          <SideNavCollapseToggle panel={panel} />
          <PaneTrail>
            <nav
              aria-label="Breadcrumb"
              className="flex shrink-0 items-center gap-2.5 text-[11px] text-muted-foreground"
            >
              <SubjectCrumbs subjectId={subject.id} />
            </nav>
            <PaneTitle>{subjectTabLabel(pathname)}</PaneTitle>
          </PaneTrail>
        </PaneHeaderRow>}

        {/* Keyed so switching subjects remounts the tab. */}
        <div key={subject.id} data-dom-find-content className="min-h-0 flex-1 overflow-hidden">
          <SubjectPanelContext.Provider value={panel}>
            <Outlet context={subject satisfies Subject} />
          </SubjectPanelContext.Provider>
        </div>
      </div>
    </SideNavShell>
  );
}

/** The subject for the current /subjects/:subjectId route. */
export function useSubject(): Subject {
  return useOutletContext<Subject>();
}

/** The subject's nav column, for a tab drawing its own header's toggle. */
export function useSubjectPanel(): Panel {
  const panel = useContext(SubjectPanelContext);
  if (!panel) throw new Error("useSubjectPanel outside SubjectLayout");
  return panel;
}
