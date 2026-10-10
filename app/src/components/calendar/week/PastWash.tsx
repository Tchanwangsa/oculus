import { minutesFromMidnight } from "@/lib/planning/calendar";
import { sameDay, startOfDay } from "@/lib/format/format";
import { HOUR_PX } from "@/components/calendar/week/constants";

/**
 * Grey over elapsed time. Mixed from muted-foreground, since `surface` is too
 * close to the page background to read in both themes.
 */
export function PastWash({
  day,
  now,
  fromHour,
  gridHeight,
}: {
  day: Date;
  now: Date;
  fromHour: number;
  gridHeight: number;
}) {
  let height = 0;
  if (sameDay(day, now)) {
    const elapsed = (minutesFromMidnight(now) - fromHour * 60) / 60;
    height = Math.min(gridHeight, Math.max(0, elapsed * HOUR_PX));
  } else if (day.getTime() < startOfDay(now).getTime()) {
    height = gridHeight;
  }
  if (height <= 0) return null;
  return (
    <div
      className="pointer-events-none absolute inset-x-0 top-0"
      style={{
        height,
        backgroundColor:
          "color-mix(in srgb, var(--color-muted-foreground) 9%, transparent)",
      }}
    />
  );
}
