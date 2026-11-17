import { sqliteUtcToMs } from "@/lib/format/format";
import { weekOf, type SubjectWeeks } from "@/lib/weeks";

export function sourceDate(value: string | null): Date | null {
  const ms = value == null ? NaN : Date.parse(value);
  return Number.isFinite(ms) ? new Date(ms) : null;
}

export function localRowDate(value: string | null): Date | null {
  const ms = sqliteUtcToMs(value);
  return ms == null ? null : new Date(ms);
}

export function inWeek(strip: SubjectWeeks, key: number | null, date: Date | null): date is Date {
  return key != null && date != null && weekOf(strip, date) === key;
}
