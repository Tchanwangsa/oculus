import { cn } from "@/lib/utils";

export type TimelineZoom = "day" | "week" | "month";

/** Pixels per day, not a CSS `zoom` (see docs/ui.md#gotchas). */
export const TIMELINE_ZOOMS = [
  { id: "day", label: "Days" },
  { id: "week", label: "Weeks" },
  { id: "month", label: "Months" },
] as const satisfies ReadonlyArray<{ id: TimelineZoom; label: string }>;

export const TIMELINE_ZOOM_KEY = "oculus-project-timeline-zoom";

export function isTimelineZoom(v: string | null): v is TimelineZoom {
  return v === "day" || v === "week" || v === "month";
}

/** Rendered in the page toolbar. Rectangular: segmented toolbars are the
 *  exception to the pill rule (same control as `CalendarPage`). */
export function TimelineZoomControl({
  value,
  onChange,
}: {
  value: TimelineZoom;
  onChange: (value: TimelineZoom) => void;
}) {
  return (
    <div className="flex shrink-0 items-center rounded-md border border-border p-0.5">
      {TIMELINE_ZOOMS.map((z) => (
        <button
          key={z.id}
          type="button"
          onClick={() => onChange(z.id)}
          className={cn(
            "cursor-pointer rounded-[4px] px-2 py-1 text-[11.5px] font-medium transition-colors",
            value === z.id
              ? "bg-surface-raised text-foreground"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {z.label}
        </button>
      ))}
    </div>
  );
}
