import { sqliteUtcToMs, startOfDay } from "@/lib/format/format";
import { displayLocale } from "@/lib/format/locale";
import type { DocumentVersion } from "@/lib/notes/documentVersions";

/** One local day of the list: its midnight, and its rows with their index in
 *  the whole list (newest first), for {@link lengthDelta}. */
export interface VersionDay {
  day: number;
  rows: { version: DocumentVersion; index: number }[];
}

/** Groups a newest-first list by the **local** day of `created_at` (stored
 *  as UTC); a row whose stamp doesn't parse joins the day above it. */
export function groupVersionsByDay(versions: DocumentVersion[]): VersionDay[] {
  const days: VersionDay[] = [];
  versions.forEach((version, index) => {
    const ms = sqliteUtcToMs(version.created_at);
    const last = days[days.length - 1];
    const day = ms != null ? startOfDay(new Date(ms)).getTime() : (last?.day ?? 0);
    if (last && last.day === day) last.rows.push({ version, index });
    else days.push({ day, rows: [{ version, index }] });
  });
  return days;
}

/** "Today", "Yesterday", else the weekday and date as `fmtDayHeading` writes
 *  them ("Thu, 11 Oct" in en-AU), with the year when it isn't this one. */
export function dayHeading(day: number, now: Date): string {
  const d = new Date(day);
  const daysAgo = Math.round((startOfDay(now).getTime() - startOfDay(d).getTime()) / 86_400_000);
  if (daysAgo === 0) return "Today";
  if (daysAgo === 1) return "Yesterday";
  const opts: Intl.DateTimeFormatOptions = { weekday: "short", day: "numeric", month: "short" };
  if (d.getFullYear() !== now.getFullYear()) opts.year = "numeric";
  return d.toLocaleDateString(displayLocale, opts);
}

/** How many characters row `index` gained over the next-older row; null for
 *  the oldest, which has nothing to compare with. */
export function lengthDelta(versions: DocumentVersion[], index: number): number | null {
  const older = versions[index + 1];
  const row = versions[index];
  return row && older ? row.length - older.length : null;
}

/** "+37", "−12" (a real minus), "±0". */
export function fmtDelta(delta: number): string {
  if (delta === 0) return "±0";
  const n = Math.abs(delta).toLocaleString(displayLocale);
  return delta > 0 ? `+${n}` : `−${n}`;
}
