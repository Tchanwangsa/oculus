import { Progress } from "@/components/ui/progress";
import { checklistProgress } from "./taskFormat";

/** A short bar and `done/total` for the checklist; not a control, so it takes
 *  no hover. Nothing while the checklist is empty. */
export function ChecklistProgress({ items }: { items: { done_at: string | null }[] }) {
  const { done, total, pct } = checklistProgress(items);
  if (total === 0) return null;
  return (
    <span
      title="Checklist progress"
      className="inline-flex h-7 shrink-0 items-center gap-2 rounded-full border border-border-subtle px-2.5 text-xs"
    >
      <Progress
        value={pct}
        aria-label="Checklist progress"
        className="h-1 w-10 shrink-0"
        indicatorClassName={pct === 100 ? "bg-success" : undefined}
      />
      <span className="tabular-nums text-muted-foreground">
        {done}/{total}
      </span>
    </span>
  );
}
