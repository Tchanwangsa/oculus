import { startOfDay } from "@/lib/format/format";

export const DAY_MS = 86_400_000;

export function addDays(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
}

export function addMonths(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth() + n, 1);
}

/** The system locale's first day of the week, 0 = Sunday … 6 = Saturday;
 *  Monday where the engine cannot say. WebKit has had both spellings of
 *  `Intl.Locale`'s week info. */
export const WEEK_START: number = (() => {
  try {
    const locale = new Intl.Locale(navigator.language) as Intl.Locale & {
      getWeekInfo?: () => { firstDay: number };
      weekInfo?: { firstDay: number };
    };
    const info = locale.getWeekInfo?.() ?? locale.weekInfo;
    // Intl counts 1 = Monday … 7 = Sunday.
    if (info) return info.firstDay % 7;
  } catch {
    /* No locale to ask. */
  }
  return 1;
})();

/** The start of `d`'s week, on the locale's first day ({@link WEEK_START}). */
export function startOfWeek(d: Date): Date {
  const day = (d.getDay() - WEEK_START + 7) % 7;
  return addDays(startOfDay(d), -day);
}

/**
 * The six weeks a month grid draws, each from {@link WEEK_START}. Always six
 * rows, so the grid never changes height as you page through months.
 */
export function monthGrid(month: Date): Date[][] {
  const first = startOfWeek(new Date(month.getFullYear(), month.getMonth(), 1));
  return Array.from({ length: 6 }, (_, w) =>
    Array.from({ length: 7 }, (_, d) => addDays(first, w * 7 + d)),
  );
}

export function weekDays(anchor: Date): Date[] {
  const first = startOfWeek(anchor);
  return Array.from({ length: 7 }, (_, i) => addDays(first, i));
}
