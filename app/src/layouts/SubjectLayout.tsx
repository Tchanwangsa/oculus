import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  NavLink,
  Navigate,
  Outlet,
  useLocation,
  useOutletContext,
  useParams,
} from "react-router-dom";
import {
  ChatsCircle,
  Folder,
  House,
  Kanban,
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
  { to: "files",         label: "Files",         icon: Folder,         end: false },
  { to: "announcements", label: "Announcements", icon: Megaphone,      end: false },
  { to: "assignments",   label: "Assignments",   icon: PencilLine,     end: false },
  { to: "discussion",    label: "Discussion",    icon: ChatsCircle,    end: false },
  { to: "projects",      label: "Projects",      icon: Kanban,         end: false },
] as const;

/**
 * Everything under /subjects/:subjectId. Resolves the subject once; child pages
 * read it via `useSubject()`. Each tab owns the full remaining height.
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
                <span className="text-[10px] uppercase tracking-wide text-muted-foreground bg-surface-raised px-2 py-0.5 rounded">
                  {subject.term_name ?? "Past"}
                </span>
              )}
            </div>
            <p className="mt-1.5 ml-[30px] text-[13px] text-muted-foreground truncate">
              {displayName(subject.name, subject.code)}
            </p>
          </div>

          <TabStrip>
            {TABS.map((tab) => {
              const newCount = newCountForTab(newCounts, subject.id, tab.to);
              return (
                <NavLink
                  key={tab.to}
                  to={tab.to}
                  end={tab.end}
                  className={({ isActive }) =>
                    cn(
                      "flex shrink-0 items-center gap-1.5 border-b-2 px-2 pb-2 pt-1 text-[12px] font-medium transition-colors",
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
          </TabStrip>
        </div>
      </header>

      {/* Keyed so switching subjects remounts the tab. Not `relative`: peeks
          inside must anchor to AppLayout's main to cover the whole page. */}
      <div key={subject.id} className="flex-1 min-h-0 overflow-hidden">
        <Outlet context={subject satisfies Subject} />
      </div>
    </div>
  );
}

/**
 * The tab row, scrolling sideways with a hidden bar and edge fades when it
 * does not fit. `-mb-px` is on the scroller, not each tab: a scroll container
 * clips both axes, so a margin inside it would clip the active underline.
 */
function TabStrip({ children }: { children: React.ReactNode }) {
  const ref = useRef<HTMLElement>(null);
  const [edges, setEdges] = useState({ left: false, right: false });
  const { pathname } = useLocation();

  const measure = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    const left = el.scrollLeft > 1;
    const right = el.scrollLeft + el.clientWidth < el.scrollWidth - 1;
    setEdges((prev) =>
      prev.left === left && prev.right === right ? prev : { left, right },
    );
  }, []);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    measure();
    el.addEventListener("scroll", measure, { passive: true });
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", measure);
      ro.disconnect();
    };
  }, [measure]);

  // Keep the active tab in view when reached from elsewhere (⌘K, restore).
  useEffect(() => {
    ref.current
      ?.querySelector('[aria-current="page"]')
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [pathname]);

  return (
    /* `flex` stops the scroller's -1px margin collapsing through the wrapper. */
    <div className="relative flex">
      <nav
        ref={ref}
        className={cn(
          "-mb-px flex min-w-0 flex-1 items-center gap-1 overflow-x-auto overflow-y-hidden",
          // A bar would jog every tab up; the fades carry the affordance.
          "[scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
        )}
      >
        {children}
      </nav>
      <StripFade side="left" show={edges.left} />
      <StripFade side="right" show={edges.right} />
    </div>
  );
}

/** Fade over one end of the strip while there is more that way. A plain
 *  gradient, not a backdrop layer, to avoid a compositing layer. */
function StripFade({ side, show }: { side: "left" | "right"; show: boolean }) {
  return (
    <div
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-y-0 w-8 transition-opacity duration-150",
        side === "left"
          ? "left-0 bg-gradient-to-r from-card via-card/85 to-transparent"
          : "right-0 bg-gradient-to-l from-card via-card/85 to-transparent",
        show ? "opacity-100" : "opacity-0",
      )}
    />
  );
}

/** The subject for the current /subjects/:subjectId route. */
export function useSubject(): Subject {
  return useOutletContext<Subject>();
}
