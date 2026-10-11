import { useMemo, useState } from "react";
import { ChartBar, Flame, Sun, Timer } from "@phosphor-icons/react";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { cn } from "@/lib/utils";
import {
  bestStreak,
  fmtUsage,
  hourStep,
  perDayUsed,
  percentChange,
  rangeTotals,
  stackBySubject,
  stackByType,
  usageStreak,
  type ChartDay,
} from "@/lib/activity/usage";
import { Change, Stat } from "@/components/home/activity/ActivityStats";
import {
  ACTIVE_FILL,
  IDLE_FILL,
  LABEL_EVERY,
  PLOT_PX,
  PRIOR,
  VIEW_KEY,
  VIEW_TABS,
  readView,
} from "@/components/home/activity/constants";
import { DayCard } from "@/components/home/activity/DayCard";
import { plural, rangeLine, shortDate } from "@/components/home/activity/format";
import { subjectSeries, typeSeries } from "@/components/home/activity/series";
import { Swatch } from "@/components/home/activity/Swatch";
import type { Segment, Series } from "@/components/home/activity/types";
import { useActivityData } from "@/components/home/activity/useActivityData";

/**
 * App use over the last 30 days: a row of stat cards from `usage_hours`, then
 * one bar per day. Time stacks active time under the rest of the open time;
 * Subject and Type split active time by what was in front, from
 * `usage_context_hours`, in a fixed stack order so segments hold their place.
 * Hovering a day opens a card over its bar with that day's figures.
 */
export function ActivityCard({ now }: { now: Date }) {
  const [hovered, setHovered] = useState<ChartDay | null>(null);
  const [view, setView] = useStoredState(VIEW_KEY, readView);
  const { days, context, colors, subjects, today, range, prior } = useActivityData(now, view);

  const totals = days && rangeTotals(days, range);
  const priorTotals = days && rangeTotals(days, prior);
  // Today against the days used before it in the window, not an average
  // that already counts today.
  const beforeToday = days && rangeTotals(days, range.slice(0, -1));
  const todayActive = days?.get(today)?.active ?? 0;
  const streak = days ? usageStreak(days, now) : 0;
  const best = days ? bestStreak(days) : 0;

  // Subject and Type draw nothing until their read lands, never the Time bars.
  const byContext = view !== "time";
  const stacked = useMemo(() => {
    if (!context || !byContext) return null;
    return view === "type" ? stackByType(context, range) : stackBySubject(context, range);
  }, [context, view, byContext, range]);

  const series = useMemo(() => {
    if (!stacked) return [];
    if (view === "type") return stacked.series.map(typeSeries);
    const byId = new Map(subjects.map((s) => [s.id, s]));
    return stacked.series.map((key) => subjectSeries(key, byId, colors));
  }, [stacked, view, subjects, colors]);

  /** The hovered day's rows, in stack order, bottom first. */
  const timeSeries: Series[] = [
    { key: "active", label: "Active", color: ACTIVE_FILL, mark: <Swatch color={ACTIVE_FILL} /> },
    { key: "idle", label: "Idle", color: IDLE_FILL, mark: <Swatch color={IDLE_FILL} /> },
  ];

  const segmentsOf = (key: string): Segment[] => {
    if (byContext && !stacked) return [];
    if (!stacked) {
      const use = days?.get(key);
      const activeSec = use?.active ?? 0;
      return [
        { key: "active", seconds: activeSec, color: ACTIVE_FILL },
        { key: "idle", seconds: Math.max(0, (use?.open ?? 0) - activeSec), color: IDLE_FILL },
      ];
    }
    const values = stacked.days.get(key) ?? [];
    return series.map((s, i) => ({ key: s.key, seconds: values[i] ?? 0, color: s.color }));
  };

  const max = byContext
    ? (stacked?.max ?? 0)
    : Math.max(0, ...range.map((d) => days?.get(d.key)?.open ?? 0));
  const step = hourStep(max) * 3600;
  const ticks = [1, 2, 3, 4].map((i) => i * step).filter((t, i) => i === 0 || t - step < max);
  const top = ticks[ticks.length - 1];
  // The px the tallest bar spends on gaps, held back from every bar's scale.
  const gapPx = 2 * (stacked ? Math.max(0, stacked.series.length - 1) : 1);
  const empty = stacked !== null && stacked.series.length === 0;

  return (
    <div className="space-y-3 self-start">
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Stat
          icon={Timer}
          label="Total active"
          value={totals && fmtUsage(totals.active)}
          detail={
            totals &&
            priorTotals && (
              <Change pct={percentChange(totals.active, priorTotals.active)} against={PRIOR} />
            )
          }
        />
        <Stat
          icon={Sun}
          label="Today"
          value={days && fmtUsage(todayActive)}
          detail={
            beforeToday && (
              <Change pct={percentChange(todayActive, perDayUsed(beforeToday))} against="average" />
            )
          }
        />
        <Stat
          icon={ChartBar}
          label="Daily average"
          value={totals && fmtUsage(perDayUsed(totals))}
          detail={
            totals &&
            priorTotals && (
              <Change
                pct={percentChange(perDayUsed(totals), perDayUsed(priorTotals))}
                against={PRIOR}
              />
            )
          }
        />
        <Stat
          icon={Flame}
          label="Streak"
          value={days && plural(streak, "day")}
          detail={days && `Highest: ${plural(best, "day")}`}
        />
      </div>

      <section className="rounded-lg border border-border p-4">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-x-4 gap-y-1">
          <div className="flex items-center gap-3">
            <h2 className="text-[13px] font-semibold text-foreground">Activity</h2>
            <PillTabs tabs={VIEW_TABS} value={view} onChange={setView} />
          </div>
          <div className="text-[12px] tabular-nums text-muted-foreground">{rangeLine(range)}</div>
        </div>

        <div className="flex gap-2" onMouseLeave={() => setHovered(null)}>
          {/* Hour labels, each centred on its gridline. The invisible top label
              sizes the column to its widest, so there is no dead space left of them. */}
          <div className="relative shrink-0 whitespace-nowrap text-[10px] tabular-nums leading-none text-muted-foreground">
            <span aria-hidden className="invisible block">
              {fmtUsage(top)}
            </span>
            {ticks.map((t) => (
              <span
                key={t}
                className="absolute right-0 translate-y-1/2"
                style={{ bottom: `${(t / top) * 100}%` }}
              >
                {fmtUsage(t)}
              </span>
            ))}
          </div>

          <div className="min-w-0 flex-1">
            <div className="relative" style={{ height: PLOT_PX }}>
              {ticks.map((t) => (
                <div
                  key={t}
                  className="absolute inset-x-0 h-px bg-border-subtle"
                  style={{ bottom: `${(t / top) * 100}%` }}
                />
              ))}
              <div className="absolute inset-x-0 bottom-0 h-px bg-border" />

              <div className="absolute inset-0 flex">
                {range.map((d) => {
                  const segments = segmentsOf(d.key);
                  // Drawn top first; only the topmost segment rounds.
                  const shown = segments.filter((s) => s.seconds > 0).reverse();
                  return (
                    // The whole column is the hover target, not just the bar.
                    <div
                      key={d.key}
                      className={cn(
                        "flex flex-1 flex-col items-center justify-end rounded-[3px]",
                        // Nothing to point at in an empty view.
                        !empty && hovered?.key === d.key && "bg-accent",
                      )}
                      onMouseEnter={() => setHovered(d)}
                    >
                      <Tooltip open={!empty && hovered?.key === d.key}>
                        <TooltipTrigger asChild>
                          <div className="flex w-[64%] max-w-6 flex-col gap-[2px]">
                            {shown.map((s, i) => (
                              <div
                                key={s.key}
                                className={cn(i === 0 && "rounded-t-[4px]")}
                                style={{ height: `${(s.seconds / top) * (PLOT_PX - gapPx)}px`, background: s.color }}
                              />
                            ))}
                          </div>
                        </TooltipTrigger>
                        {/* Pointer-transparent, so moving onto it never ends the hover. */}
                        <TooltipContent side="top" sideOffset={4} className="pointer-events-none">
                          <DayCard
                            day={d}
                            series={byContext ? series : timeSeries}
                            values={segments.map((s) => s.seconds)}
                          />
                        </TooltipContent>
                      </Tooltip>
                    </div>
                  );
                })}
              </div>

              {/* Above the columns, so a hover never paints over it. */}
              {empty && (
                <p className="pointer-events-none absolute inset-0 flex items-center justify-center text-[12px] text-muted-foreground">
                  Tracking by {view} started recently
                </p>
              )}
            </div>

            <div className="relative mt-1.5 flex h-3 text-[10px] leading-none text-muted-foreground">
              {range.map((d, i) => (
                <div key={d.key} className="relative flex-1">
                  {(range.length - 1 - i) % LABEL_EVERY === 0 && (
                    <span className="absolute left-1/2 -translate-x-1/2 whitespace-nowrap">
                      {shortDate(d.date)}
                    </span>
                  )}
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* One row of fixed height whatever the view, so switching never
            resizes the card; long labels truncate. */}
        <div className="mt-3 flex h-4 items-center justify-end gap-3 overflow-hidden text-[10px] leading-4 text-muted-foreground">
          {byContext ? (
            series.map((s) => (
              <span key={s.key} className="flex min-w-0 items-center gap-1" title={s.label}>
                {s.legendMark ?? <Swatch color={s.color} />}
                <span className="truncate">{s.label}</span>
              </span>
            ))
          ) : (
            <>
              <span className="flex items-center gap-1">
                <Swatch color={ACTIVE_FILL} />
                Active
              </span>
              <span className="flex items-center gap-1">
                <Swatch color={IDLE_FILL} />
                Idle
              </span>
            </>
          )}
        </div>
      </section>
    </div>
  );
}
