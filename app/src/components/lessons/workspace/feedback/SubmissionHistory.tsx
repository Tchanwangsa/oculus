import { Check, X } from "@phosphor-icons/react";

import { submissionSummary } from "@/lib/ed/feedback";
import { fmtRecent } from "@/lib/format/format";
import type { Submission } from "@/lib/ed/workspace";
import { cn } from "@/lib/utils";

/** The student's submissions as Ed's table lists them, newest first: its
 *  number, when it went in, testcases passed and the result. Picking one
 *  shows it. */
export function SubmissionHistory({
  submissions,
  selected,
  onSelect,
}: {
  submissions: Submission[];
  selected: number;
  onSelect: (id: number) => void;
}) {
  return (
    <section aria-label="Submissions" className="space-y-1">
      <h3 className="text-xs font-semibold text-muted-foreground">Submissions</h3>
      <div role="table" className="text-xs">
        <div role="row" className="flex items-center gap-3 px-2 py-1 text-muted-foreground">
          <span role="columnheader" className="w-8 shrink-0">
            #
          </span>
          <span role="columnheader" className="min-w-0 flex-1">
            Submitted
          </span>
          <span role="columnheader" className="w-16 shrink-0 text-right">
            Testcases
          </span>
          <span role="columnheader" className="w-12 shrink-0 text-right">
            Result
          </span>
        </div>
        {submissions.map((sub, i) => {
          const { passed, total } = submissionSummary(sub);
          const at = Date.parse(sub.createdAt);
          const current = sub.id === selected;
          return (
            <button
              key={sub.id}
              type="button"
              role="row"
              aria-current={current}
              onClick={() => onSelect(sub.id)}
              className={cn(
                "flex w-full cursor-pointer items-center gap-3 rounded-md px-2 py-1 text-left transition-colors",
                current ? "bg-secondary text-foreground" : "text-muted-foreground hover:bg-accent hover:text-foreground",
              )}
            >
              <span role="cell" className="w-8 shrink-0 tabular-nums">
                {submissions.length - i}
              </span>
              <span role="cell" className="min-w-0 flex-1 truncate">
                {Number.isFinite(at) ? fmtRecent(at) : sub.status}
              </span>
              <span role="cell" className="w-16 shrink-0 text-right tabular-nums">
                {sub.result ? `${passed} / ${total}` : "—"}
              </span>
              <span role="cell" className="flex w-12 shrink-0 justify-end">
                {!sub.result ? (
                  <span>{sub.status}</span>
                ) : sub.result.passed ? (
                  <Check className="text-success" aria-label="Passed" />
                ) : (
                  <X className="text-destructive" aria-label="Failed" />
                )}
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
