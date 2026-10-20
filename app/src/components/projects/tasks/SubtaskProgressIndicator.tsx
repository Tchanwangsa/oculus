import { cn } from "@/lib/utils";
import type { SubtaskProgress } from "./taskTree";

/** A parent's full checklist progress, including children hidden by filters. */
export function SubtaskProgressIndicator({ progress }: { progress: SubtaskProgress }) {
  const { done, total, pct } = progress;
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-border px-2 py-0.5 text-[11px] tabular-nums text-muted-foreground">
      <svg aria-hidden="true" width="14" height="14" viewBox="0 0 20 20" className={cn("shrink-0", done === total && "text-success")}>
        <circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" strokeWidth="2" opacity="0.25" />
        <circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" strokeWidth="2" pathLength="100" strokeDasharray={`${pct} 100`} strokeLinecap="round" transform="rotate(-90 10 10)" />
      </svg>
      <span>{done}/{total}</span>
    </span>
  );
}
