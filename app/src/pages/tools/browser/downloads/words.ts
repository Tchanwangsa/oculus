import { isMac, isWindows } from "@/lib/platform";

/** "On this Mac…" / "On this PC…", as the Ed workspace names the machine. */
export const THIS_COMPUTER = isMac ? "Mac" : "PC";

/** The file manager's reveal, as Studio's export and Ed's run files word it. */
export const REVEAL = isWindows ? "Show in Explorer" : "Show in Finder";

/** A rejected command's message, for an inline error. */
export function problem(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
