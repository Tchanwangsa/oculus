import { parseToolMeta, toolVerb } from "./meta";
import type { HarnessItem } from "./types";

/** A path's last segment, so a read names the file and not its folders. */
const lastSegment = (s: string) => (/^[~./\w-]*\/[^\s]*$/.test(s) ? s.replace(/\/+$/, "").split("/").pop() || s : s);

/**
 * What a working run is doing now, as the thread list's third line: its
 * newest row while that is a tool still running ("Reading week-7.pdf") or
 * reasoning ("Thinking"). Null when the newest row is anything else, so the
 * row falls back to the last message. Only a thread some view holds has rows.
 */
export function activityLine(items: HarnessItem[] | undefined): string | null {
  const last = items?.[items.length - 1];
  if (!last) return null;
  if (last.kind === "thinking") return "Thinking";
  if (last.kind !== "tool") return null;
  const meta = parseToolMeta(last);
  if (meta.ok != null) return null;
  const verb = toolVerb(meta.kind ?? "other", false, meta.name);
  const what = (last.content ?? "").split("\n")[0].trim();
  return what ? `${verb} ${lastSegment(what)}` : verb;
}
