import { startOfWeek } from "@/lib/planning/calendar/dates";
import type { SubjectWeeks } from "./types";

export function weekKey(date: Date): number {
  const monday = startOfWeek(date);
  return Date.UTC(monday.getFullYear(), monday.getMonth(), monday.getDate());
}

export function mondayFromKey(key: number): Date {
  const civil = new Date(key);
  return new Date(civil.getUTCFullYear(), civil.getUTCMonth(), civil.getUTCDate());
}

/** Dates outside the strip fold into its edge weeks. Invalid dates and empty
 *  strips have no week; callers can keep undated material separately. */
export function weekOf(strip: SubjectWeeks, date: Date): number | null {
  if (strip.weeks.length === 0 || !Number.isFinite(date.getTime())) return null;
  const key = weekKey(date);
  const first = strip.weeks[0];
  const last = strip.weeks[strip.weeks.length - 1];
  if (key <= first.key) return first.key;
  if (key >= last.key) return last.key;
  return strip.weeks.find((week) => week.key === key)?.key ?? null;
}
