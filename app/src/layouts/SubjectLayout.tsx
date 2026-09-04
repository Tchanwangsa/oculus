import { useEffect, useMemo } from "react";
import {
  NavLink,
  Navigate,
  Outlet,
  useOutletContext,
  useParams,
} from "react-router-dom";
import {
  ChatsCircle,
  DownloadSimple,
  House,
  Megaphone,
  PencilLine,
  Stack,
  VideoCamera,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useSubjects } from "@/hooks/useSubjects";
import { NewCountBadge } from "@/components/NewCountBadge";
import { newCountForTab, useNewFilesStore } from "@/stores/newFilesStore";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { SubjectIconPicker } from "@/components/subjects/SubjectIconPicker";
import { displayCode, displayName } from "@/lib/format";
import { Skeleton } from "@/components/ui/skeleton";
import type { Subject } from "@/lib/db";

const TABS = [
  { to: ".",             label: "Overview",      icon: House,          end: true },
  { to: "modules",       label: "Modules",       icon: Stack,          end: false },
  { to: "lectures",      label: "Lectures",      icon: VideoCamera,    end: false },
  { to: "downloads",     label: "Downloads",     icon: DownloadSimple, end: false },
  { to: "announcements", label: "Announcements", icon: Megaphone,      end: false },
  { to: "assignments",   label: "Assignments",   icon: PencilLine,     end: false },
  { to: "discussion",    label: "Discussion",    icon: ChatsCircle,    end: false },
] as const;

/**
 * Everything under /subjects/:subjectId. Resolves the id once here so the child
 * pages never load or pick a subject themselves — they read it off the outlet
 * context via `useSubject()`.
 *
 * The header is the subject's identity (large title) plus the tab strip; each
 * tab then owns the entire remaining height, which is what the Files and
 * Lectures three-pane views need.
 */
export default function SubjectLayout() {
  const { subjectId } = useParams();
  const id = Number(subjectId);
  const { subjects, loading } = useSubjects();
  const newCounts = useNewFilesStore((s) => s.bySubject);

  const subject = useMemo(
    () => subjects.find((s) => s.id === id) ?? null,
    [subjects, id],
  );

  useEffect(() => {
    if (subject) document.title = `${subject.code} · Oculus`;
    return () => {
      document.title = "Oculus";
    };
  }, [subject]);

  if (!Number.isFinite(id)) return <Navigate to="/subjects" replace />;

  if (loading && !subject) {
    return (
      <div className="mx-auto max-w-5xl px-6 pt-6 space-y-3">
        <Skeleton className="h-7 w-52" />
        <Skeleton className="h-4 w-72" />
      </div>
    );
  }

  // Loaded but no such subject — a stale id, e.g. after clearing the DB.
  if (!subject) return <Navigate to="/subjects" replace />;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="shrink-0 border-b border-border-subtle">
        {/* Same centered column as the tab content below it. */}
        <div className="mx-auto max-w-5xl px-6">
          <div className="pt-5 pb-3">
            <div className="flex items-center gap-2.5">
              <SubjectIconPicker code={subject.code}>
                <button
                  type="button"
                  aria-label="Change subject icon"
                  className="-m-1 rounded-md p-1 hover:bg-surface transition-colors"
                >
                  <SubjectIcon code={subject.code} size={20} />
                </button>
              </SubjectIconPicker>
              <h1 className="text-[22px] font-semibold tracking-tight text-foreground leading-none">
                {displayCode(subject.code)}
              </h1>
              {!subject.is_current && (
                <span className="text-[10px] uppercase tracking-wide text-muted-foreground font-mono bg-surface-raised px-2 py-0.5 rounded">
                  {subject.term_name ?? "Past"}
                </span>
              )}
            </div>
            <p className="mt-1.5 ml-[30px] text-[13px] text-muted-foreground truncate">
              {displayName(subject.name, subject.code)}
            </p>
          </div>

          <nav className="flex items-center gap-1">
            {TABS.map((tab) => {
              const newCount = newCountForTab(newCounts, subject.id, tab.to);
              return (
                <NavLink
                  key={tab.to}
                  to={tab.to}
                  end={tab.end}
                  className={({ isActive }) =>
                    cn(
                      // -1px bottom margin so the active underline sits on the
                      // header's border rather than above it.
                      "-mb-px flex items-center gap-1.5 border-b-2 px-2 pb-2 pt-1 text-[12px] font-medium transition-colors",
                      isActive
                        ? "border-primary text-foreground"
                        : "border-transparent text-muted-foreground hover:text-foreground",
                    )
                  }
                >
                  {({ isActive }) => (
                    <>
                      <tab.icon
                        size={13}
                        weight={isActive ? "fill" : "regular"}
                        className={cn("shrink-0", isActive && "text-primary")}
                      />
                      {tab.label}
                      <NewCountBadge count={newCount} />
                    </>
                  )}
                </NavLink>
              );
            })}
          </nav>
        </div>
      </header>

      {/* Keyed on the subject so switching subjects remounts the tab instead of
          carrying the previous subject's open file / playing lecture across.
          Deliberately NOT position:relative — peeks rendered inside must anchor
          to AppLayout's main so they overlay the whole page. */}
      <div key={subject.id} className="flex-1 min-h-0 overflow-hidden">
        <Outlet context={subject satisfies Subject} />
      </div>
    </div>
  );
}

/** The subject for the current /subjects/:subjectId route. */
export function useSubject(): Subject {
  return useOutletContext<Subject>();
}
