import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { ArrowRight, CaretRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useSubjects } from "@/hooks/useSubjects";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, displayName, fmtSynced } from "@/lib/format";
import { newCountForSubject, useNewFilesStore } from "@/stores/newFilesStore";
import type { Subject } from "@/lib/db";

/** The landing page for /subjects — pick a subject, then work inside it. */
export default function SubjectsIndexPage() {
  const { subjects, loading, current, past } = useSubjects();
  const navigate = useNavigate();
  const [pastOpen, setPastOpen] = useState(false);

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-5xl px-6 py-6">
        <h1 className="text-[22px] font-semibold tracking-tight text-foreground leading-none">
          Subjects
        </h1>
        <p className="mt-1.5 text-[13px] text-muted-foreground">
          Everything synced from Canvas, one home per subject.
        </p>
        <div className="mt-5 border-t border-border" />

        {loading && (
          <div className="mt-6 grid grid-cols-1 sm:grid-cols-2 gap-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} className="h-24 w-full" />
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
          <section className="mt-7">
            <h2 className="mb-2.5 text-[13px] font-semibold text-foreground">
              Current subjects
            </h2>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              {current.map((s) => (
                <SubjectCard key={s.id} subject={s} />
              ))}
            </div>
          </section>
        )}

        {past.length > 0 && (
          <section className="mt-8">
            <button
              type="button"
              onClick={() => setPastOpen((v) => !v)}
              aria-expanded={pastOpen}
              // Not an <h2> — a heading element can't live inside a button —
              // so it borrows the base heading rule's font and tracking.
              className="flex items-center gap-1.5 font-display text-[13px] font-semibold tracking-[-0.02em] text-foreground hover:opacity-70 transition-opacity"
            >
              <CaretRight
                size={11}
                className={cn("transition-transform", pastOpen && "rotate-90")}
              />
              Past subjects
              <span className="font-normal text-muted-foreground/70 tabular-nums">
                {past.length}
              </span>
            </button>
            {pastOpen && (
              <div className="mt-3 grid grid-cols-1 sm:grid-cols-2 gap-3">
                {past.map((s) => (
                  <SubjectCard key={s.id} subject={s} dimmed />
                ))}
              </div>
            )}
          </section>
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
  const bySubject = useNewFilesStore((s) => s.bySubject);
  const newCount = newCountForSubject(bySubject, subject.id);

  return (
    <Link
      to={`/subjects/${subject.id}`}
      className={cn(
        "group rounded-xl border border-border px-4 py-3.5 hover:bg-surface transition-colors",
        dimmed && "opacity-70 hover:opacity-100",
      )}
    >
      <div className="flex items-center gap-2.5">
        <SubjectIcon code={subject.code} size={16} />
        <span className="font-display text-[13px] font-semibold text-foreground truncate">
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
      <p className="mt-2 ml-[26px] flex items-center gap-1.5 text-[11px] text-muted-foreground/70">
        {newCount > 0 && (
          <>
            <span className="size-1.5 shrink-0 rounded-full bg-brand" />
            <span className="font-medium text-brand tabular-nums">
              {newCount} new {newCount === 1 ? "item" : "items"}
            </span>
            <span aria-hidden>·</span>
          </>
        )}
        <span className="truncate">{fmtSynced(subject.last_synced_at)}</span>
      </p>
    </Link>
  );
}
