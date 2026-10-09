import type { ChartDay } from "@/lib/activity/usage";

export function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** "6 Oct". */
export function shortDate(d: Date): string {
  return d.toLocaleDateString("en-AU", { day: "numeric", month: "short" });
}

/** "7 Sep – 6 Oct". */
export function rangeLine(range: ChartDay[]): string {
  return `${shortDate(range[0].date)} – ${shortDate(range[range.length - 1].date)}`;
}
