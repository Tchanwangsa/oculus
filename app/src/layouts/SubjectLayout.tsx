import { useEffect, useMemo } from "react";
import { Navigate, Outlet, useLocation, useOutletContext, useParams } from "react-router-dom";
import { useResizablePanel } from "@/hooks/useResizablePanel";
import { useSubjects } from "@/hooks/useSubjects";
import { SubjectCrumbs } from "@/components/subjects/SubjectCrumbs";
import {
  SUBJECT_NAV_PANEL,
  SubjectNav,
  SubjectNavSkeleton,
  subjectTabLabel,
} from "@/components/subjects/SubjectNav";
import { PaneHeaderRow, PaneTitle, PaneTrail } from "@/components/tabs/PaneHeader";
import { SideNavCollapseToggle } from "@/components/ui/SideNav";
import type { Subject } from "@/lib/db";

const LAST_SUBJECT_KEY = "oculus-last-subject";

/** The subject a page last resolved, for `/subjects` to reopen
 *  (`app/src/pages/SubjectsRedirect.tsx`). The caller checks it still exists. */
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
  const panel = useResizablePanel(SUBJECT_NAV_PANEL);
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
      <div className="flex h-full overflow-hidden">
        <SubjectNavSkeleton />
      </div>
    );
  }

  // Loaded but no such subject — a stale id, e.g. after clearing the DB. The
  // redirect skips a remembered id that is gone, so it never bounces back.
  if (!subject) return <Navigate to="/subjects" replace />;

  return (
    <div className="flex h-full overflow-hidden">
      <SubjectNav subject={subject} current={current} past={past} panel={panel} />

      <div className="flex h-full min-w-0 flex-1 flex-col">
        <PaneHeaderRow className="flex h-12 shrink-0 items-center gap-2.5 border-b border-border-subtle px-6">
          <SideNavCollapseToggle collapsed={panel.collapsed} onToggle={panel.toggle} shortcut="⌘⌥B" />
          <PaneTrail>
            <nav
              aria-label="Breadcrumb"
              className="flex shrink-0 items-center gap-2.5 text-[11px] text-muted-foreground"
            >
              <SubjectCrumbs subjectId={subject.id} />
            </nav>
            <PaneTitle>{subjectTabLabel(pathname)}</PaneTitle>
          </PaneTrail>
        </PaneHeaderRow>

        {/* Keyed so switching subjects remounts the tab. */}
        <div key={subject.id} className="min-h-0 flex-1 overflow-hidden">
          <Outlet context={subject satisfies Subject} />
        </div>
      </div>
    </div>
  );
}

/** The subject for the current /subjects/:subjectId route. */
export function useSubject(): Subject {
  return useOutletContext<Subject>();
}
