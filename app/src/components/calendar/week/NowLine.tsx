import { minutesFromMidnight } from "@/lib/planning/calendar";
import { HOUR_PX } from "@/components/calendar/week/constants";

/** The current time across today's column, only while inside the grid's hours. */
export function NowLine({ fromHour, toHour }: { fromHour: number; toHour: number }) {
  const now = new Date();
  const mins = minutesFromMidnight(now);
  if (mins < fromHour * 60 || mins > toHour * 60) return null;
  const top = ((mins - fromHour * 60) / 60) * HOUR_PX;
  return (
    <div
      className="pointer-events-none absolute inset-x-0 z-10 border-t border-destructive"
      style={{ top }}
    >
      <span className="absolute -left-1 -top-[3px] block h-1.5 w-1.5 rounded-full bg-destructive" />
    </div>
  );
}
