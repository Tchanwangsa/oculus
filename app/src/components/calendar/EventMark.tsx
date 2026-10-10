import { CheckSquare, Flag, PushPin } from "@phosphor-icons/react";
import type { CalKind } from "@/lib/planning/calendar";

/**
 * The mark leading every event in every view: flag for a deadline, pin for a
 * note, checkbox for a task, dot for anything that occupies time.
 */
export function EventMark({
  kind,
  color,
  size,
}: {
  kind: CalKind;
  color: string;
  /** Icon size in px; the dot is drawn slightly smaller to match weight. */
  size: number;
}) {
  if (kind === "due") {
    return <Flag size={size} weight="fill" className="shrink-0" style={{ color }} />;
  }
  if (kind === "note") {
    return <PushPin size={size} weight="fill" className="shrink-0" style={{ color }} />;
  }
  // Outline, not the flag's solid fill: a self-set task date must not read as
  // a Canvas cutoff at a glance.
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
