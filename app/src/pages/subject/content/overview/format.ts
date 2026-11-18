import { fmtDay, fmtShortDate, fmtTime, startOfDay } from "@/lib/format/format";
import { displayLocale } from "@/lib/format/locale";
import type { Week } from "@/lib/weeks";

const DAY_MS = 86_400_000;

/** "Mon". */
export function weekday(d: Date): string {
  return d.toLocaleDateString(displayLocale, { weekday: "short" });
}

/** A row's day: "Mon" inside one week, "12 Nov" in an edge week spanning many. */
export function rowDay(d: Date, open: boolean): string {
  return open ? fmtDay(d) : weekday(d);
}

/** Which end of the strip a week sits at. `weekOf` folds every date before
 *  the strip into its first week and every date after it into its last. */
export type WeekEdge = "first" | "last" | null;

export function weekEdge(index: number, count: number): WeekEdge {
  return index === count - 1 ? "last" : index === 0 ? "first" : null;
}

/** "22 Sep – 28 Sep": the week's Monday to its Sunday (`end` is the next Monday).
 *  The edge weeks hold everything beyond them: "Until 28 Sep", "From 22 Sep". */
export function weekRange(week: Week, edge: WeekEdge = null): string {
  const sunday = fmtShortDate(new Date(week.end.getTime() - DAY_MS));
  if (edge === "last") return `From ${fmtShortDate(week.start)}`;
  if (edge === "first") return `Until ${sunday}`;
  return `${fmtShortDate(week.start)} – ${sunday}`;
}

/** What the strip's heading calls a week: "Week 5", "Break", "2 weeks after teaching",
 *  and for the last one, which holds everything later, "3+ weeks after teaching". */
export function weekName(week: Week, edge: WeekEdge = null): string {
  if (week.kind === "teaching") return `Week ${week.label}`;
  if (week.kind === "break") return "Break";
  const n = Number(week.label.replace("+", ""));
  if (edge === "last") return `${n}+ weeks after teaching`;
  return n === 1 ? "1 week after teaching" : `${n} weeks after teaching`;
}

export function daysFrom(now: Date, d: Date): number {
  return Math.round((startOfDay(d).getTime() - startOfDay(now).getTime()) / DAY_MS);
}

/** A deadline's slot: "Today · 5:00 pm", "Tomorrow · 11:59 pm", "Fri · 4 days",
 *  or with the date once it is a week or more away. */
export function dueLabel(date: Date, now: Date): string {
  const days = daysFrom(now, date);
  if (days <= 0) return `Today · ${fmtTime(date.getTime())}`;
  if (days === 1) return `Tomorrow · ${fmtTime(date.getTime())}`;
  const day = days < 7 ? weekday(date) : `${weekday(date)} ${fmtShortDate(date)}`;
  return `${day} · ${days} days`;
}

/** An event's slot: "Today 2:15 pm", "Mon 4:15 pm" ("12 Nov 4:15 pm" in an edge
 *  week), or the day alone when all-day. */
export function eventLabel(date: Date, allDay: boolean, now: Date, open = false): string {
  const day = daysFrom(now, date) === 0 ? "Today" : rowDay(date, open);
  return allDay ? day : `${day} ${fmtTime(date.getTime())}`;
}

/** "Lecture 13", "Lecture 13 and Lecture 14", "Lecture 12, Lecture 13 and 2 more". */
export function lectureList(titles: string[]): string {
  if (titles.length <= 2) return titles.join(" and ");
  if (titles.length === 3) return `${titles[0]}, ${titles[1]} and ${titles[2]}`;
  return `${titles[0]}, ${titles[1]} and ${titles.length - 2} more`;
}
