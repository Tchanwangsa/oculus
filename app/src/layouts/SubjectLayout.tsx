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
  DownloadSimple,
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
  { to: "downloads",     label: "Downloads",     icon: DownloadSimple, end: false },
  { to: "announcements", label: "Announcements", icon: Megaphone,      end: false },
  { to: "assignments",   label: "Assignments",   icon: PencilLine,     end: false },
  { to: "discussion",    label: "Discussion",    icon: ChatsCircle,    end: false },
  { to: "projects",      label: "Projects",      icon: Kanban,         end: false },
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

/**
 * The tab row, as a strip that scrolls sideways rather than one that gets cut
 * off. Eight tabs already crowd the centred column, and the card is narrower
 * still whenever the side panel is docked open — so the row is a scroller with
 * its bar hidden and a fade over each live edge, which is the affordance the
 * sidebar's own scroller uses.
 *
 * `-mb-px` sits on the scroller rather than on each tab: `overflow-x` makes
 * this a scroll container, and a scroll container clips on *both* axes, so a
 * negative bottom margin inside it would take the active underline with it.
 * On the container itself the margin is outside the clip, and the underline
 * still lands on the header's border instead of above it.
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
    // The strip's own width is what changes — the side panel opening, the
    // sidebar folding — so watch the scroller rather than its contents.
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", measure);
      ro.disconnect();
    };
  }, [measure]);

  // A tab reached from anywhere but the strip — ⌘K, a card on the overview,
  // a restored tab — may be sitting off the end; the strip follows it.
  useEffect(() => {
    ref.current
      ?.querySelector('[aria-current="page"]')
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [pathname]);

  return (
    /* `flex` rather than a plain block: it stops the scroller's -1px bottom
       margin collapsing out through the wrapper, so the underline's overlap
       with the header border is the same 1px it always was. */
    <div className="relative flex">
      <nav
        ref={ref}
        className={cn(
          "-mb-px flex min-w-0 flex-1 items-center gap-1 overflow-x-auto overflow-y-hidden",
          // The bar would take a 6px gutter out of a 30px-tall row and jog
          // every tab upward; the fades carry the affordance instead.
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

/** Fade over one end of the strip, shown only while there is more that way.
 *  A plain gradient over the card's ground, for the reason `ScrollFade` in the
 *  sidebar is one: a backdrop layer here would cost a compositing layer. */
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
