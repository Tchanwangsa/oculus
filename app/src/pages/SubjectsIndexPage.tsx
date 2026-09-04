import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { ArrowRight, CaretRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useSubjects } from "@/hooks/useSubjects";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, displayName } from "@/lib/format";
import type { Subject } from "@/lib/db";

/** The landing page for /subjects — pick a subject, then work inside it. */
export default function SubjectsIndexPage() {
  const { subjects, loading, current, past } = useSubjects();
  const navigate = useNavigate();
  const [pastOpen, setPastOpen] = useState(false);

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-5xl px-6 py-6">
        <h1 className="text-[22px] font-semibold tracking-tight text-foreground leading-none">
          Subjects
        </h1>
        <p className="mt-1.5 text-[13px] text-muted-foreground">
          Everything synced from Canvas, one home per subject.
        </p>

        {loading && (
          <div className="mt-6 grid grid-cols-1 sm:grid-cols-2 gap-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} className="h-20 w-full" />
            ))}
          </div>
        )}

        {!loading && subjects.length === 0 && (
          <div className="mt-6 rounded-lg border border-border px-4 py-6 text-center">
            <p className="text-sm text-foreground">No subjects yet.</p>
            <p className="text-xs text-muted-foreground mt-1">
              Sign in and run a sync to pull your Canvas courses.
            </p>
            <Button
              size="sm"
              variant="outline"
              className="mt-3 text-xs"
              onClick={() => navigate("/sync")}
            >
              Go to Sync
            </Button>
          </div>
        )}

        {current.length > 0 && (
          <div className="mt-6 grid grid-cols-1 sm:grid-cols-2 gap-3">
            {current.map((s) => (
              <SubjectCard key={s.id} subject={s} />
            ))}
          </div>
        )}

        {past.length > 0 && (
          <div className="mt-8">
            <button
              type="button"
              onClick={() => setPastOpen((v) => !v)}
              aria-expanded={pastOpen}
              className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground hover:text-foreground transition-colors"
            >
              <CaretRight
                size={10}
                className={cn("transition-transform", pastOpen && "rotate-90")}
              />
              Past subjects ({past.length})
            </button>
            {pastOpen && (
              <div className="mt-3 grid grid-cols-1 sm:grid-cols-2 gap-3">
                {past.map((s) => (
                  <SubjectCard key={s.id} subject={s} dimmed />
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function SubjectCard({
  subject,
  dimmed = false,
}: {
  subject: Subject;
  dimmed?: boolean;
}) {
  return (
    <Link
      to={`/subjects/${subject.id}`}
      className={cn(
        "group rounded-lg border border-border px-4 py-3.5 hover:bg-surface transition-colors",
        dimmed && "opacity-70 hover:opacity-100",
      )}
    >
      <div className="flex items-center gap-2.5">
        <SubjectIcon code={subject.code} size={16} />
        <span className="text-[13px] font-semibold text-foreground truncate">
          {displayCode(subject.code)}
        </span>
        <ArrowRight
          size={13}
          className="ml-auto shrink-0 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity"
        />
      </div>
      <p className="mt-1.5 ml-[26px] text-[12px] text-muted-foreground line-clamp-2 leading-snug">
        {displayName(subject.name, subject.code)}
      </p>
      {subject.term_name && (
        <p className="mt-1.5 ml-[26px] text-[10px] uppercase tracking-wide font-mono text-muted-foreground/70">
          {subject.term_name}
        </p>
      )}
    </Link>
  );
}
