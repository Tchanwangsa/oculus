import { Flag, PushPin } from "@phosphor-icons/react";
import type { CalKind } from "@/lib/calendar";

/**
 * The mark that leads every event, in every view: a flag for a deadline, a pin
 * for a note Oculus wrote, a dot for anything that occupies time.
 *
 * Shared because the three views draw the same vocabulary and had already
 * grown three copies of the flag-or-dot branch — a fourth kind would have
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
  const dot = Math.round(size * 0.7);
  return (
    <span
      className="shrink-0 rounded-full"
      style={{ width: dot, height: dot, backgroundColor: color }}
    />
  );
}
