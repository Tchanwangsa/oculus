import { submissionSummary } from "@/lib/ed/feedback";
import { fmtRecent } from "@/lib/format/format";
import type { Submission } from "@/lib/ed/workspace";
import { cn } from "@/lib/utils";

/** The submissions this app made, newest first; picking one shows it. */
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
      <ul className="space-y-0.5">
        {submissions.map((sub) => {
          const { passed, total } = submissionSummary(sub);
          const at = Date.parse(sub.createdAt);
          return (
            <li key={sub.id}>
              <button
                type="button"
                aria-current={sub.id === selected}
                onClick={() => onSelect(sub.id)}
                className={cn(
                  "flex w-full cursor-pointer items-center gap-3 rounded-md px-2 py-1 text-left text-xs transition-colors",
                  sub.id === selected ? "bg-secondary text-foreground" : "text-muted-foreground hover:bg-accent hover:text-foreground",
                )}
              >
                <span className="min-w-0 flex-1 truncate">{Number.isFinite(at) ? fmtRecent(at) : sub.status}</span>
                <span className="shrink-0 tabular-nums">
                  {sub.result ? `${passed} / ${total}` : sub.status}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
