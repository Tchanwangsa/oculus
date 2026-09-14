import { CheckSquare, Flag, PushPin } from "@phosphor-icons/react";
import type { CalKind } from "@/lib/calendar";

/**
 * The mark that leads every event, in every view: a flag for a deadline, a pin
 * for a note Oculus wrote, a checkbox for a project task, a dot for anything
 * that occupies time.
 *
 * Shared because the three views draw the same vocabulary and had already
 * grown three copies of the flag-or-dot branch — a fifth kind would have
 * drifted between them.
 */
export function EventMark({
  kind,
  color,
  size,
}: {
  kind: CalKind;
  color: string;
  /** Icon size in px; the dot is drawn a little smaller so the two read as the
   *  same weight beside each other. */
  size: number;
}) {
  if (kind === "due") {
    return <Flag size={size} weight="fill" className="shrink-0" style={{ color }} />;
  }
  if (kind === "note") {
    return <PushPin size={size} weight="fill" className="shrink-0" style={{ color }} />;
  }
  // A checkbox, drawn as an outline where the deadline's flag is a solid fill.
  // A task's date is one you set yourself, and it must not read as a Canvas
  // cutoff at a glance: a different silhouette says which layer it is, and the
  // lighter ink says it carries less consequence than a submission does.
  if (kind === "task") {
    return <CheckSquare size={size} className="shrink-0" style={{ color }} />;
  }
  const dot = Math.round(size * 0.7);
  return (
    <span
      className="shrink-0 rounded-full"
      style={{ width: dot, height: dot, backgroundColor: color }}
    />
  );
}
