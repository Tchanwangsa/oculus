import { useRef, type KeyboardEvent } from "react";
import type { WeekProgress } from "@/hooks/data/overview";
import type { SubjectWeeks } from "@/lib/weeks";
import { weekEdge } from "./format";
import { WeekChip } from "./WeekChip";

const STEP: Record<string, number> = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 };

/**
 * Every week of the term: numbered teaching weeks, wider dashed Breaks, then
 * +1 to +3 after a hairline. The current week is marked in the brand colour;
 * the selected one is filled. Arrows, Home and End move the selection, which
 * keeps the one tab stop (a radio group).
 */
export function WeekStrip({ strip, progress, selected, onSelect }: {
  strip: SubjectWeeks;
  progress: WeekProgress[];
  /** Index into `strip.weeks`. */
  selected: number;
  onSelect: (key: number) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const byKey = new Map(progress.map((week) => [week.weekKey, week]));
  const last = strip.weeks.length - 1;

  const onKeyDown = (event: KeyboardEvent) => {
    const next = event.key === "Home" ? 0
      : event.key === "End" ? last
      : event.key in STEP ? Math.min(last, Math.max(0, selected + STEP[event.key]))
      : null;
    if (next == null) return;
    event.preventDefault();
    onSelect(strip.weeks[next].key);
    ref.current?.querySelector<HTMLElement>(`[data-week="${next}"]`)?.focus();
  };

  return (
    <div ref={ref} role="radiogroup" aria-label="Weeks" onKeyDown={onKeyDown} className="flex min-w-0 gap-[3px]">
      {strip.weeks.map((week, i) => (
        <WeekChip
          key={week.key}
          index={i}
          week={week}
          edge={weekEdge(i, strip.weeks.length)}
          progress={byKey.get(week.key)}
          current={i === strip.current}
          future={strip.current >= 0 && i > strip.current}
          selected={i === selected}
          divided={week.kind === "after" && strip.weeks[i - 1]?.kind !== "after" && i > 0}
          onSelect={onSelect}
        />
      ))}
    </div>
  );
}
