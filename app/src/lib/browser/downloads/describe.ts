import { fmtBytes } from "@/lib/format/format";
import type { Dest, Download, SavedTo } from "./types";

/**
 * A download's state in words, for its row in the list. A place is returned
 * apart from the words so the row can draw a subject as the rest of the app
 * does (its icon and short code), never the raw Canvas code.
 */

/** Where a download is going or went: a subject's Uploads, or a folder. */
export type Place =
  | { kind: "subject"; code: string }
  | { kind: "folder"; name: string };

export type Tone = "muted" | "brand" | "success" | "error";

export interface StateWords {
  /** Leads the line: "Receiving", "Will save to", "Saved to"… */
  text: string;
  place: Place | null;
  /** Why it failed or was refused, or a saved file's conversion problem. */
  detail: string | null;
  tone: Tone;
}

/** The last folder of `path`, either separator: what "saved to" names. */
export function folderOf(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.length > 1 ? parts[parts.length - 2] : path;
}

function placeOf(dest: Dest | SavedTo): Place | null {
  switch (dest.kind) {
    case "oculus": return { kind: "subject", code: dest.subject_code };
    case "path": return { kind: "folder", name: folderOf(dest.path) };
    case "discard": return null;
  }
}

export function stateWords(d: Download): StateWords {
  switch (d.state) {
    case "receiving":
      return d.choice
        ? { text: "Will save to", place: placeOf(d.choice), detail: null, tone: "brand" }
        : { text: "Receiving", place: null, detail: null, tone: "brand" };
    case "ready":
      return d.error
        ? { text: "Not saved", place: null, detail: d.error, tone: "error" }
        : { text: "Waiting for a choice", place: null, detail: null, tone: "brand" };
    case "delivering":
      return { text: "Saving to", place: d.choice ? placeOf(d.choice) : null, detail: null, tone: "brand" };
    case "saved": {
      const saved = d.saved_to;
      // It landed even when its conversion for search failed.
      const problem = saved?.kind === "oculus" ? saved.outcome.error : null;
      return { text: "Saved to", place: saved ? placeOf(saved) : null, detail: problem, tone: "success" };
    }
    case "discarded":
      return { text: "Discarded", place: null, detail: null, tone: "muted" };
    case "failed":
      return { text: "Failed", place: null, detail: d.error, tone: "error" };
    case "refused":
      return { text: "Not downloaded", place: null, detail: d.error, tone: "error" };
  }
}

/** The size beside a download: bytes so far while it arrives (there is no
 *  total to give), the final size after; null for one that was refused,
 *  which has no bytes to count. */
export function sizeWords(d: Download): string | null {
  if (d.state === "refused") return null;
  return d.state === "receiving" ? `${fmtBytes(d.received)} received` : fmtBytes(d.received);
}
