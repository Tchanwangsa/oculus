import { useStoredState } from "@/hooks/useStoredState";
import { useState } from "react";
import { CaretRight, DotsThree } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { navigateActive } from "@/lib/tabRouters";
import { useActivePath } from "@/stores/tabStore";
import { useSubjects } from "@/hooks/useSubjects";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { NewCountBadge } from "@/components/NewCountBadge";
import { newCountForSubject, useNewFilesStore } from "@/stores/newFilesStore";
import { displayCode } from "@/lib/format";
import type { Subject } from "@/lib/db";

const OPEN_KEY = "oculus-subjects-nav-open";
const PAST_KEY = "oculus-subjects-nav-past-shown";
/** How many more past subjects each "More" click reveals. */
const PAST_STEP = 5;

/* Outside the component so collapsing the sidebar (a remount) keeps it; written
   synchronously, not from an effect that could be torn down first. */
let pastShownCache = ((): number => {
  const n = Number(localStorage.getItem(PAST_KEY));
  return Number.isFinite(n) && n > 0 ? n : 0;
})();

function storePastShown(n: number) {
  pastShownCache = n;
  localStorage.setItem(PAST_KEY, String(n));
}

/** Header navigates to the subject index; the caret expands the list in place. */
export default function SubjectsNavGroup() {
  const { current, past, loading } = useSubjects();
  const [open, setOpen] = useStoredState(OPEN_KEY, (stored) => stored !== "false");
  const [pastShown, setPastShown] = useState(() => pastShownCache);

  // Only on the index itself; inside a subject its own row lights instead.
  const inSection = useActivePath().split("?")[0] === "/subjects";


  return (
    <div>
      {/* Label navigates; the hover caret (over the count) collapses. */}
      <div className="group/row flex items-center justify-between pl-2 pr-1 mb-0.5">
        <button
          type="button"
          data-tab-href="/subjects"
          onClick={() => navigateActive("/subjects")}
          className={cn(
            "flex-1 min-w-0 truncate py-1 text-left text-[11px] font-medium tracking-wide transition-colors",
            inSection
              ? "text-foreground"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          Subjects
        </button>
        <button
          type="button"
          onClick={() => {
            const next = !open;
            setOpen(next);
            // Collapsing folds the past subjects away again.
            if (!next) {
              storePastShown(0);
              setPastShown(0);
            }
          }}
          aria-label={open ? "Collapse subjects" : "Expand subjects"}
          aria-expanded={open}
          className="flex h-5 w-5 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground transition-colors"
        >
          <CaretRight
            size={11}
            className={cn(
              "hidden group-hover/row:block transition-transform",
              open && "rotate-90",
            )}
          />
          {!loading && current.length > 0 && (
            <span className="text-[10px] tabular-nums opacity-60 group-hover/row:hidden">
              {current.length}
            </span>
          )}
        </button>
      </div>

      {open && (
        <div className="space-y-0.5">
          {loading && (
            <p className="pl-2 py-1 text-[12px] text-muted-foreground">Loading…</p>
          )}

          {!loading && current.length === 0 && past.length === 0 && (
            <p className="pl-2 py-1 text-[12px] text-muted-foreground">
              None synced
            </p>
          )}

          {current.map((s) => (
            <SubjectNavRow key={s.id} subject={s} />
          ))}

          {past.length > 0 && (
            <>
              {past.slice(0, pastShown).map((s) => (
                <SubjectNavRow key={s.id} subject={s} dimmed />
              ))}
              {/* "More" reads as a row, unfolding past subjects a few at a time. */}
              <button
                type="button"
                onClick={() => {
                  const next =
                    pastShown >= past.length
                      ? 0
                      : Math.min(pastShown + PAST_STEP, past.length);
                  storePastShown(next);
                  setPastShown(next);
                }}
                className="w-full flex items-center gap-2.5 rounded-md pl-2 pr-1.5 py-1.5 text-[12.5px] text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground transition-colors"
              >
                <DotsThree size={15} weight="bold" className="shrink-0" />
                <span className="truncate flex-1 text-left">
                  {pastShown >= past.length ? "Less" : "More"}
                </span>
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function SubjectNavRow({
  subject,
  dimmed = false,
}: {
  subject: Subject;
  dimmed?: boolean;
}) {
  const bySubject = useNewFilesStore((s) => s.bySubject);
  const newCount = newCountForSubject(bySubject, subject.id);
  const to = `/subjects/${subject.id}`;
  // Exact match (`NavLink`'s `end`), so a lecture or file tab doesn't light it.
  const isActive = useActivePath().split("?")[0] === to;

  return (
    <button
      type="button"
      data-tab-href={to}
      onClick={() => navigateActive(to)}
      title={subject.name}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-md pl-2 pr-1.5 py-1.5 text-[12.5px] transition-colors",
        isActive
          ? "bg-sidebar-item-active text-foreground font-medium"
          : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground",
        dimmed && "opacity-60",
      )}
    >
      <SubjectIcon code={subject.code} size={15} />
      <span className="truncate flex-1 text-left">{displayCode(subject.code)}</span>
      <NewCountBadge count={newCount} />
    </button>
  );
}
