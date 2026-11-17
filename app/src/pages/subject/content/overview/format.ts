import { fmtShortDate, fmtTime, startOfDay } from "@/lib/format/format";
import { displayLocale } from "@/lib/format/locale";
import type { Week } from "@/lib/weeks";

const DAY_MS = 86_400_000;

/** "Mon". */
export function weekday(d: Date): string {
  return d.toLocaleDateString(displayLocale, { weekday: "short" });
}

/** "22 Sep – 28 Sep": the week's Monday to its Sunday (`end` is the next Monday). */
export function weekRange(week: Week): string {
  return `${fmtShortDate(week.start)} – ${fmtShortDate(new Date(week.end.getTime() - DAY_MS))}`;
}

/** What the strip's heading calls a week: "Week 5", "Break", "2 weeks after teaching". */
export function weekName(week: Week): string {
  if (week.kind === "teaching") return `Week ${week.label}`;
  if (week.kind === "break") return "Break";
  const n = Number(week.label.replace("+", ""));
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

/** An event's slot: "Today 2:15 pm", "Mon 4:15 pm", or the day alone when all-day. */
export function eventLabel(date: Date, allDay: boolean, now: Date): string {
  const day = daysFrom(now, date) === 0 ? "Today" : weekday(date);
  return allDay ? day : `${day} ${fmtTime(date.getTime())}`;
}

/** "Lecture 13", "Lecture 13 and Lecture 14", "Lecture 12, Lecture 13 and 2 more". */
export function lectureList(titles: string[]): string {
  if (titles.length <= 2) return titles.join(" and ");
  if (titles.length === 3) return `${titles[0]}, ${titles[1]} and ${titles[2]}`;
  return `${titles[0]}, ${titles[1]} and ${titles.length - 2} more`;
}
