import { fmtUsage, type ChartDay } from "@/lib/activity/usage";
import type { Series } from "@/components/home/activity/types";

/** "Tue 6 Oct" over each series with time that day: its mark, label and time. */
export function DayCard({ day, series, values }: { day: ChartDay; series: Series[]; values: number[] }) {
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
