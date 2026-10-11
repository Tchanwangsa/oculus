import {
  BookOpen,
  Chat,
  FileText,
  Globe,
  House,
  VideoCamera,
  type Icon,
} from "@phosphor-icons/react";
import { CALENDAR_UPDATED_EVENT } from "@/lib/planning/calendar";
import { PROJECTS_UPDATED_EVENT } from "@/lib/planning/projects";
import { CHART_DAYS, type TypeGroup } from "@/lib/activity/usage";

/** The pings land in `usage_hours` with no event, so the card polls. */
export const EVENTS: string[] = [];
export const POLL_MS = 5 * 60_000;

/** Subject colours come from the calendar's rows, which change on sync and on
 *  task writes — the same events `UpcomingCard` re-reads on. */
export const COLOR_EVENTS = [CALENDAR_UPDATED_EVENT, PROJECTS_UPDATED_EVENT];

export type View = "time" | "subject" | "type";

export const VIEW_TABS: ReadonlyArray<{ value: View; label: string }> = [
  { value: "time", label: "Time" },
  { value: "subject", label: "Subject" },
  { value: "type", label: "Type" },
];

export const VIEW_KEY = "oculus-home-activity-view";

export const readView = (raw: string | null): View =>
  VIEW_TABS.some((t) => t.value === raw) ? (raw as View) : "time";

/** Active time is the brand; the rest of the open time a tint of it, stacked
 *  above, so a bar's full height is the time the window was open. Tints mix
 *  with the card, not `transparent`, so a gridline never shows through a bar. */
export const ACTIVE_FILL = "var(--color-brand)";
export const IDLE_FILL = "color-mix(in srgb, var(--color-brand) 28%, var(--color-card))";

/** Plot height in px; bars are sized in px against it, less the 2px gap a
 *  stacked bar spends between its segments. */
export const PLOT_PX = 144;

/** What the 30-day figures compare against. */
export const PRIOR = `prior ${CHART_DAYS} days`;

/** A date label under every seventh bar, counted back from today's. */
export const LABEL_EVERY = 7;

export const OTHER_FILL = "var(--color-chart-other)";

/** The Type view's series, in `TYPE_GROUPS` order: chart colours in sequence,
 *  the catch-all grey. */
export const TYPE_SERIES: Record<TypeGroup, { label: string; icon: Icon; color: string }> = {
  lectures: { label: "Lectures", icon: VideoCamera, color: "var(--color-chart-1)" },
  files: { label: "Files & notes", icon: FileText, color: "var(--color-chart-2)" },
  course: { label: "Course pages", icon: BookOpen, color: "var(--color-chart-3)" },
  chat: { label: "Chat", icon: Chat, color: "var(--color-chart-4)" },
  browser: { label: "Browser", icon: Globe, color: "var(--color-chart-5)" },
  // Pages outside the kinds above: Home, planning, settings, sync.
  other: { label: "General", icon: House, color: OTHER_FILL },
};
