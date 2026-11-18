import type { WeekProgress } from "@/hooks/data/overview";
import type { Week } from "@/lib/weeks";
import { cn } from "@/lib/utils";
import { weekName, weekRange, type WeekEdge } from "./format";

/** Breaks and the weeks after teaching have no lectures to fill a bar. */
const DASHED = "bg-[repeating-linear-gradient(90deg,var(--color-border)_0_3px,transparent_3px_6px)]";

export function WeekChip({ index, week, edge, progress, current, future, selected, divided, onSelect }: {
  index: number;
  week: Week;
  edge: WeekEdge;
  progress: WeekProgress | undefined;
  current: boolean;
  future: boolean;
  selected: boolean;
  /** The hairline before +1. */
  divided: boolean;
  onSelect: (key: number) => void;
}) {
  const watched = progress && progress.total > 0
    ? `${progress.watched} of ${progress.total} ${progress.total === 1 ? "lecture" : "lectures"} watched`
    : null;
  const range = weekRange(week, edge);
  const tip = [current ? "This week" : weekName(week, edge), range, watched].filter(Boolean).join(" · ");
  return (
    <>
      {divided && <span aria-hidden className="mx-[3px] my-1.5 w-px shrink-0 bg-border" />}
      <button
        type="button"
        role="radio"
        aria-checked={selected}
        aria-current={current ? "date" : undefined}
        aria-label={tip}
        title={tip}
        tabIndex={selected ? 0 : -1}
        data-week={index}
        onClick={() => onSelect(week.key)}
        className={cn(
          "flex h-7.5 min-w-0 flex-1 flex-col items-center justify-center gap-[3px] overflow-hidden rounded-md text-[10.5px] leading-none tabular-nums outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-ring/50",
          week.kind === "break" && "flex-[1.5]",
          week.kind === "break" && !selected && "italic",
          selected && (current ? "bg-brand-muted" : "bg-surface-raised"),
          !selected && "hover:bg-surface",
          current ? "font-semibold text-brand"
            : selected ? "font-semibold text-foreground"
            : future ? "text-muted-foreground/50"
            : "text-muted-foreground",
        )}
      >
        <span className="max-w-full overflow-hidden px-0.5 whitespace-nowrap">{week.label}</span>
        <Bar week={week} progress={progress} current={current} />
      </button>
    </>
  );
}

function Bar({ week, progress, current }: { week: Week; progress: WeekProgress | undefined; current: boolean }) {
  if (week.kind !== "teaching") return <span aria-hidden className={cn("h-[3px] w-3/5 rounded-full", DASHED)} />;
  const share = progress && progress.total > 0 ? progress.watched / progress.total : 0;
  return (
    <span aria-hidden className="relative h-[3px] w-3/5 overflow-hidden rounded-full bg-border">
      {share > 0 && (
        <span
          className={cn("absolute inset-y-0 left-0 rounded-full", current ? "bg-brand" : "bg-brand/45")}
          style={{ width: `${Math.round(share * 100)}%` }}
        />
      )}
    </span>
  );
}
