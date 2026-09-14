import { useEffect, useState } from "react";
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

/* How far the past list is unfolded lives outside the component so that
   remounting the sidebar — collapsing it with ⌘\ — doesn't lose it. Writes go
   through here synchronously rather than from an effect, which could be torn
   down before it ever ran. Collapsing the Subjects group itself is different:
   that folds the past subjects away deliberately and resets this to 0. */
let pastShownCache = ((): number => {
  const n = Number(localStorage.getItem(PAST_KEY));
  return Number.isFinite(n) && n > 0 ? n : 0;
})();

function storePastShown(n: number) {
  pastShownCache = n;
  localStorage.setItem(PAST_KEY, String(n));
}

/**
 * The Subjects group: a header row that navigates to the subject index, plus a
 * caret that expands the list of subjects in place.
 */
export default function SubjectsNavGroup() {
  const { current, past, loading } = useSubjects();
  const [open, setOpen] = useState(
    () => localStorage.getItem(OPEN_KEY) !== "false",
  );
  // Past subjects unfold a page at a time rather than all at once.
  const [pastShown, setPastShown] = useState(() => pastShownCache);

  // Active only on the subject index itself — inside an individual subject the
  // subject's own row carries the highlight instead.
  const inSection = useActivePath().split("?")[0] === "/subjects";

  useEffect(() => {
    localStorage.setItem(OPEN_KEY, String(open));
  }, [open]);

  return (
    <div>
      {/* Notion-style section header: a small muted label, no icon. The label
          navigates to the subject index; the caret (revealed on hover, where
          the count sits) collapses the list. */}
      <div className="group/row flex items-center justify-between pl-2 pr-1 mb-0.5">
        <button
          type="button"
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
            // Collapsing the group is a fresh start: the past subjects fold
            // back away, so re-expanding comes back to just the current ones.
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
              {/* Notion-style "More" row: sits under the last subject and reads
                  as one of them, so past subjects unfold in place — a few at a
                  time, collapsing back once they are all out. */}
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
  // An exact match, which is `NavLink`'s `end`: the subject itself, not
  // everything under it. Without it a lecture or a file open in this subject —
  // a page of its own, with its own tab — lit the subject row as though you
  // were sitting in the subject.
  const isActive = useActivePath().split("?")[0] === to;

  return (
    <button
      type="button"
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
