import { useEffect, useState } from "react";
import { NavLink, useMatch } from "react-router-dom";
import { CaretRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useSubjects } from "@/hooks/useSubjects";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { NewCountBadge } from "@/components/NewCountBadge";
import { newCountForSubject, useNewFilesStore } from "@/stores/newFilesStore";
import { displayCode } from "@/lib/format";
import type { Subject } from "@/lib/db";

const OPEN_KEY = "oculus-subjects-nav-open";
const PAST_KEY = "oculus-subjects-nav-past-open";

/**
 * The Subjects group: a header row that navigates to the subject index, plus a
 * caret that expands the list of subjects in place.
 */
export default function SubjectsNavGroup() {
  const { current, past, loading } = useSubjects();
  const [open, setOpen] = useState(
    () => localStorage.getItem(OPEN_KEY) !== "false",
  );
  const [pastOpen, setPastOpen] = useState(
    () => localStorage.getItem(PAST_KEY) === "true",
  );

  // Active only on the subject index itself — inside an individual subject the
  // subject's own row carries the highlight instead.
  const inSection = useMatch("/subjects") != null;

  useEffect(() => {
    localStorage.setItem(OPEN_KEY, String(open));
  }, [open]);
  useEffect(() => {
    localStorage.setItem(PAST_KEY, String(pastOpen));
  }, [pastOpen]);

  return (
    <div>
      {/* Notion-style section header: a small muted label, no icon. The label
          navigates to the subject index; the caret (revealed on hover, where
          the count sits) collapses the list. */}
      <div className="group/row flex items-center justify-between pl-2 pr-1 mb-1">
        <NavLink
          to="/subjects"
          end
          className={cn(
            "flex-1 min-w-0 truncate py-1 text-[11px] font-medium transition-colors",
            inSection
              ? "text-foreground"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          Subjects
        </NavLink>
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          aria-label={open ? "Collapse subjects" : "Expand subjects"}
          aria-expanded={open}
          className="flex h-5 w-5 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground transition-colors"
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
        <div className="space-y-px">
          {loading && (
            <p className="pl-2 py-1 text-[12px] text-muted-foreground">Loading…</p>
          )}

          {!loading && current.length === 0 && past.length === 0 && (
            <p className="pl-2 py-1 text-[12px] text-muted-foreground">
              None synced
            </p>
          )}

          {current.map((s) => (
            <SubjectNavLink key={s.id} subject={s} />
          ))}

          {past.length > 0 && (
            <>
              <button
                type="button"
                onClick={() => setPastOpen((v) => !v)}
                aria-expanded={pastOpen}
                className="w-full flex items-center gap-1.5 pl-2 pr-2 py-1.5 text-[12px] font-medium text-muted-foreground hover:text-foreground transition-colors"
              >
                <CaretRight
                  size={9}
                  className={cn("shrink-0 transition-transform", pastOpen && "rotate-90")}
                />
                Past ({past.length})
              </button>
              {pastOpen &&
                past.map((s) => <SubjectNavLink key={s.id} subject={s} dimmed />)}
            </>
          )}
        </div>
      )}
    </div>
  );
}

function SubjectNavLink({
  subject,
  dimmed = false,
}: {
  subject: Subject;
  dimmed?: boolean;
}) {
  const bySubject = useNewFilesStore((s) => s.bySubject);
  const newCount = newCountForSubject(bySubject, subject.id);

  return (
    <NavLink
      to={`/subjects/${subject.id}`}
      title={subject.name}
      className={({ isActive }) =>
        cn(
          "flex items-center gap-2.5 rounded-md pl-2 pr-2 py-1.5 text-[13px] transition-colors",
          isActive
            ? "bg-sidebar-item-active text-foreground font-medium"
            : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground",
          dimmed && "opacity-60",
        )
      }
    >
      <SubjectIcon code={subject.code} size={16} />
      <span className="truncate flex-1">{displayCode(subject.code)}</span>
      <NewCountBadge count={newCount} />
    </NavLink>
  );
}
