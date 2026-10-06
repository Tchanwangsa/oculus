import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  ArrowDown,
  ArrowUp,
  BookOpen,
  ChartBar,
  Chat,
  DotsThree,
  FileText,
  Flame,
  Globe,
  House,
  Sun,
  Timer,
  VideoCamera,
  type Icon,
} from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useTabActive } from "@/components/tabs/TabContext";
import { PillTabs } from "@/components/ui/PillTabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useStoredState } from "@/hooks/useStoredState";
import { useSubjects } from "@/hooks/useSubjects";
import { CALENDAR_UPDATED_EVENT, loadCalendar, subjectColors } from "@/lib/calendar";
import type { Subject } from "@/lib/db";
import { displayCode } from "@/lib/format";
import { PROJECTS_UPDATED_EVENT } from "@/lib/projects";
import { cn } from "@/lib/utils";
import {
  CHART_DAYS,
  NO_SUBJECT_SERIES,
  OTHER_SUBJECTS,
  bestStreak,
  chartDays,
  dayKey,
  fmtUsage,
  hourStep,
  loadUsageContext,
  loadUsageDays,
  perDayUsed,
  percentChange,
  rangeTotals,
  stackBySubject,
  stackByType,
  usageStreak,
  type ChartDay,
  type TypeGroup,
  type UsageContextRow,
  type UsageDay,
} from "@/lib/usage";
import { useHomeSection } from "./useHomeSection";

/** The pings land in `usage_hours` with no event, so the card polls. */
const EVENTS: string[] = [];
const POLL_MS = 5 * 60_000;

/** Subject colours come from the calendar's rows, which change on sync and on
 *  task writes — the same events `UpcomingCard` re-reads on. */
const COLOR_EVENTS = [CALENDAR_UPDATED_EVENT, PROJECTS_UPDATED_EVENT];

type View = "time" | "subject" | "type";

const VIEW_TABS: ReadonlyArray<{ value: View; label: string }> = [
  { value: "time", label: "Time" },
  { value: "subject", label: "Subject" },
  { value: "type", label: "Type" },
];

const VIEW_KEY = "oculus-home-activity-view";

const readView = (raw: string | null): View =>
  VIEW_TABS.some((t) => t.value === raw) ? (raw as View) : "time";

/** Active time is the brand; the rest of the open time a tint of it, stacked
 *  above, so a bar's full height is the time the window was open. Tints mix
 *  with the card, not `transparent`, so a gridline never shows through a bar. */
const ACTIVE_FILL = "var(--color-brand)";
const IDLE_FILL = "color-mix(in srgb, var(--color-brand) 28%, var(--color-card))";

/** Plot height in px; bars are sized in px against it, less the 2px gap a
 *  stacked bar spends between its segments. */
const PLOT_PX = 144;

/** What the 30-day figures compare against. */
const PRIOR = `prior ${CHART_DAYS} days`;

/** A date label under every seventh bar, counted back from today's. */
const LABEL_EVERY = 7;

const OTHER_FILL = "var(--color-chart-other)";

/** The Type view's series, in `TYPE_GROUPS` order: chart colours in sequence,
 *  the catch-all grey. */
const TYPE_SERIES: Record<TypeGroup, { label: string; icon: Icon; color: string }> = {
  lectures: { label: "Lectures", icon: VideoCamera, color: "var(--color-chart-1)" },
  files: { label: "Files & notes", icon: FileText, color: "var(--color-chart-2)" },
  course: { label: "Course pages", icon: BookOpen, color: "var(--color-chart-3)" },
  chat: { label: "Chat", icon: Chat, color: "var(--color-chart-4)" },
  browser: { label: "Browser", icon: Globe, color: "var(--color-chart-5)" },
  // Pages outside the kinds above: Home, planning, settings, sync.
  other: { label: "General", icon: House, color: OTHER_FILL },
};

/** One stacked series as the legend and the hover card draw it. */
interface Series {
  key: string;
  label: string;
  color: string;
  /** The legend's mark in place of a plain swatch. */
  legendMark?: ReactNode;
  /** The hover card's mark before the label. */
  mark: ReactNode;
}

/** A bar segment, bottom first. */
interface Segment {
  key: string;
  seconds: number;
  color: string;
}

/**
 * App use over the last 30 days: a row of stat cards from `usage_hours`, then
 * one bar per day. Time stacks active time under the rest of the open time;
 * Subject and Type split active time by what was in front, from
 * `usage_context_hours`, in a fixed stack order so segments hold their place.
 * Hovering a day opens a card over its bar with that day's figures.
 */
export function ActivityCard({ now }: { now: Date }) {
  const [days, setDays] = useState<Map<string, UsageDay> | null>(null);
  const [context, setContext] = useState<UsageContextRow[] | null>(null);
  const [hovered, setHovered] = useState<ChartDay | null>(null);
  const [view, setView] = useStoredState(VIEW_KEY, readView);
  const [colors, setColors] = useState<Map<number, string>>(new Map());
  const { subjects } = useSubjects();
  const active = useTabActive();

  // Keyed on the date, so the minute tick of `now` doesn't rebuild the range.
  const today = dayKey(now);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const range = useMemo(() => chartDays(now), [today]);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const prior = useMemo(() => chartDays(now, 1), [today]);

  const reload = useCallback(() => {
    loadUsageDays(new Date())
      .then(setDays)
      .catch((e) => {
        console.error(e);
        setDays(new Map());
      });
    loadUsageContext(new Date())
      .then(setContext)
      .catch((e) => {
        console.error(e);
        setContext([]);
      });
  }, []);

  useHomeSection(reload, EVENTS);

  // Only the Subject view needs the calendar, so the read waits for it.
  const reloadColors = useCallback(() => {
    if (view !== "subject") return;
    loadCalendar()
      .then((events) => setColors(subjectColors(events)))
      .catch(console.error);
  }, [view]);

  useHomeSection(reloadColors, COLOR_EVENTS);

  useEffect(() => {
    if (!active) return;
    const id = setInterval(reload, POLL_MS);
    return () => clearInterval(id);
  }, [active, reload]);

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

/** One active-time figure under its icon and label, with a comparison or a
 *  note below; blank until the first read lands, so the row holds its height. */
function Stat({
  icon: Glyph,
  label,
  value,
  detail,
}: {
  icon: Icon;
  label: string;
  value: string | null | undefined;
  detail: ReactNode;
}) {
  return (
    <div className="rounded-lg border border-border px-3.5 py-3">
      <p className="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
        <Glyph className="size-3.5" />
        {label}
      </p>
      <p className="mt-1 h-6 text-[20px] font-semibold leading-6 tabular-nums text-foreground">
        {value}
      </p>
      <div className="mt-0.5 h-4 truncate text-[11px] leading-4 tabular-nums text-muted-foreground">
        {detail}
      </div>
    </div>
  );
}

/** "↑ 12% vs prior 30 days"; "No data" with nothing earlier to compare. */
function Change({ pct, against }: { pct: number | null; against: string }) {
  if (pct === null) return <>No data</>;
  const Arrow = pct > 0 ? ArrowUp : pct < 0 ? ArrowDown : null;
  return (
    <span className="flex min-w-0 items-center gap-1">
      <span
        className={cn(
          "flex shrink-0 items-center gap-0.5 font-medium",
          pct > 0 && "text-success",
          pct < 0 && "text-destructive",
        )}
      >
        {Arrow && <Arrow weight="bold" className="size-3" />}
        {Math.abs(pct)}%
      </span>
      <span className="truncate">vs {against}</span>
    </span>
  );
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** "6 Oct". */
function shortDate(d: Date): string {
  return d.toLocaleDateString("en-AU", { day: "numeric", month: "short" });
}

/** "7 Sep – 6 Oct". */
function rangeLine(range: ChartDay[]): string {
  return `${shortDate(range[0].date)} – ${shortDate(range[range.length - 1].date)}`;
}

/** "Tue 6 Oct" over each series with time that day: its mark, label and time. */
function DayCard({ day, series, values }: { day: ChartDay; series: Series[]; values: number[] }) {
  const date = day.date.toLocaleDateString("en-AU", {
    weekday: "short",
    day: "numeric",
    month: "short",
  });
  const rows = series
    .map((s, i) => ({ s, seconds: values[i] ?? 0 }))
    .filter((r) => r.seconds > 0);
  return (
    <div className="min-w-32">
      <p className="font-medium">{date}</p>
      {rows.length === 0 ? (
        <p className="mt-0.5 opacity-70">No activity</p>
      ) : (
        <div className="mt-1 space-y-0.5">
          {rows.map(({ s, seconds }) => (
            <div key={s.key} className="flex items-center gap-1.5">
              {s.mark}
              <span className="min-w-0 flex-1 truncate">{s.label}</span>
              <span className="pl-2 tabular-nums">{fmtUsage(seconds)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function Swatch({ color }: { color: string }) {
  return <span className="size-2.5 shrink-0 rounded-[2px]" style={{ background: color }} />;
}

/** A type group's series: its icon in its colour stands in for the swatch. */
function typeSeries(key: string): Series {
  const t = TYPE_SERIES[key as TypeGroup];
  return {
    key,
    label: t.label,
    color: t.color,
    legendMark: <t.icon weight="fill" className="size-3 shrink-0" style={{ color: t.color }} />,
    mark: <t.icon weight="fill" className="size-3.5" style={{ color: t.color }} />,
  };
}

/**
 * A subject's series: its code, in the calendar's colour for it, and its glyph
 * in the hover card. A subject with no calendar rows takes a chart colour by
 * id, which stays the same from day to day.
 */
function subjectSeries(
  key: string,
  byId: Map<number, Subject>,
  colors: Map<number, string>,
): Series {
  if (key === NO_SUBJECT_SERIES || key === OTHER_SUBJECTS) {
    const none = key === NO_SUBJECT_SERIES;
    const Glyph = none ? House : DotsThree;
    // Folded subjects take a tint of the grey, so the two never read as one.
    const color = none ? OTHER_FILL : `color-mix(in srgb, ${OTHER_FILL} 50%, var(--color-card))`;
    return {
      key,
      // Time on pages outside any subject: Home, chat, the browser, planning.
      label: none ? "General" : "Other subjects",
      color,
      mark: (
        <>
          <Swatch color={color} />
          <Glyph className="size-3" />
        </>
      ),
    };
  }
  const id = Number(key);
  const subject = byId.get(id);
  const color = colors.get(id) ?? `var(--color-chart-${(id % 5) + 1})`;
  return {
    key,
    label: subject ? displayCode(subject.code) : "Unknown subject",
    color,
    mark: (
      <>
        <Swatch color={color} />
        {subject && <SubjectIcon code={subject.code} size={12} />}
      </>
    ),
  };
}
