import type { ParseFailure, ParseLatch } from "@/stores/parseStore";

/**
 * What a file's markdown situation *is*, in one vocabulary.
 *
 * MinerU cloud is the only parser and there is no fallback beneath it, so a
 * failure is permanent until something changes: the file simply has no
 * markdown, and with it no search, no `@`-mention and no Markdown view. That
 * used to be invisible — a file row said `"failed"` or nothing at all, and the
 * viewer just didn't draw its toggle — which read as "this file is fine".
 *
 * Three questions decide which state a file is in, and they are the three the
 * `parse-status` error carries (`app/src/stores/parseStore.ts`):
 *
 * - is it moving (queued / running) or done (`quality`)?
 * - did *this file* fail, and **could a retry ever work** (`retryable`)?
 * - is the cause **the whole library's** (`latching`) — no token, a rejected
 *   token, a spent quota — rather than this file's?
 *
 * The last one is the distinction the UI must never fudge: "this file is
 * broken" and "parsing is down for everything" call for different words and
 * different fixes. `retryable` and `latching` are both **optional** — a
 * failure inherited from a previous session is only the word `error` in the
 * DB, with no column for the discriminants — so unknown is its own case and is
 * never coerced into either extreme.
 */
export type ParseStateKind =
  | "parsed"
  | "running"
  | "queued"
  | "unparsed"
  | "failed"
  | "permanent"
  | "blocked";

export interface ParseState {
  kind: ParseStateKind;
  /** The one word a file row shows. */
  label: string;
  /** The heading of the explanation, where there is room for one. */
  title: string;
  /** The explanation — the backend's own sentence whenever it gave one. */
  detail: string;
  /** How loud the row's word is allowed to be. Only a genuine failure is
   *  `bad`; "not parsed yet" is not a failure and colouring it red is a lie. */
  tone: "quiet" | "progress" | "good" | "bad" | "hold";
  /** True when Settings → Library is where this gets fixed (a missing or
   *  rejected token). A spent quota heals on its own and gets no button. */
  fixInSettings: boolean;
}

/** The background sweep's promise, and only said when it can keep it: under a
 *  latch `useQualitySweep` stands down entirely, and a file whose failure said
 *  a retry cannot work is never re-kicked. */
export const PARSE_SWEEP_NOTE = "Oculus retries outstanding files in the background.";

/**
 * Is this latching cause a token the student can fix in Settings → Library?
 *
 * The other latching causes are not: a spent quota heals on a clock, and a
 * backend/app version mismatch is an update, not a setting — pointing either
 * at Settings would send someone to a page that cannot help. `kind` is
 * preferred but optional, so the backend's own sentence is the fallback: both
 * credential messages name the token, and no other one does.
 */
function tokenish(kind: string | undefined, message: string): boolean {
  if (kind) return /credential|token/i.test(kind);
  return /token/i.test(message);
}

/**
 * Derive the state of one PDF-backed file.
 *
 * `status` is the live/DB word, `failure` what this file last failed with (if
 * this session saw it fail), `latch` the app-wide condition in force.
 */
export function parseStateOf(
  status: string | undefined,
  failure: ParseFailure | undefined,
  latch: ParseLatch | null,
): ParseState {
  // Done and moving both beat any stale failure — the store already clears a
  // file's failure the moment it moves again, so these can never disagree.
  if (status === "quality") {
    return {
      kind: "parsed",
      label: "parsed",
      title: "Parsed",
      detail: "This PDF has markdown, so it is searchable and can be mentioned in chat.",
      tone: "good",
      fixInSettings: false,
    };
  }
  if (status === "running") {
    return {
      kind: "running",
      label: "parsing",
      title: "Parsing now",
      detail: "MinerU is reading this PDF. The Markdown view appears when it finishes.",
      tone: "progress",
      fixInSettings: false,
    };
  }
  if (status === "queued") {
    return {
      kind: "queued",
      label: "queued",
      title: "Queued to parse",
      detail: "This PDF is in line to be parsed. The Markdown view appears when it finishes.",
      tone: "progress",
      fixInSettings: false,
    };
  }

  if (failure) {
    if (failure.latching) {
      return {
        kind: "blocked",
        label: "on hold",
        title: "Parsing is unavailable",
        detail: failure.message,
        tone: "hold",
        fixInSettings: tokenish(failure.kind, failure.message),
      };
    }
    if (failure.retryable === false) {
      return {
        kind: "permanent",
        label: "can't parse",
        title: "This PDF cannot be parsed",
        // No sweep line: a file whose failure said a retry cannot work is
        // never re-kicked, and promising one would be a lie.
        detail: `${failure.message} It will not be tried again.`,
        tone: "bad",
        fixInSettings: false,
      };
    }
    return {
      kind: "failed",
      label: "failed",
      title: "Parse failed",
      detail: latch ? failure.message : `${failure.message} ${PARSE_SWEEP_NOTE}`,
      tone: "bad",
      fixInSettings: false,
    };
  }

  // `error` with no discriminants: this session never saw the failure, so all
  // the DB kept is the word. Unknown retryability is still swept (that is the
  // recovery path's whole point), so the promise holds.
  if (status === "error") {
    return {
      kind: "failed",
      label: "failed",
      title: "Parse failed",
      detail: latch
        ? "An earlier attempt to parse this PDF failed, and parsing is currently unavailable."
        : `An earlier attempt to parse this PDF failed, so it has no markdown. ${PARSE_SWEEP_NOTE}`,
      tone: "bad",
      fixInSettings: false,
    };
  }

  // Nothing wrong with this file — the library's parser is down, which is why
  // it has no markdown and why nothing is coming for it.
  if (latch) {
    return {
      kind: "blocked",
      label: "on hold",
      title: "Parsing is unavailable",
      detail: latch.message,
      tone: "hold",
      fixInSettings: tokenish(latch.kind, latch.message),
    };
  }

  return {
    kind: "unparsed",
    label: "not parsed",
    title: "Not parsed yet",
    detail: `This PDF has no markdown yet, so it is not searchable and cannot be mentioned in chat. ${PARSE_SWEEP_NOTE}`,
    tone: "quiet",
    fixInSettings: false,
  };
}

/** Tailwind colour for a state's word. */
export const PARSE_TONE_CLASS: Record<ParseState["tone"], string> = {
  quiet: "text-muted-foreground/70",
  progress: "text-brand",
  good: "text-success",
  bad: "text-destructive",
  hold: "text-warning",
};
