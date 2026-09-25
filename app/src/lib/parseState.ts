import type { ParseFailure, ParseLatch } from "@/stores/parseStore";

/**
 * A file's markdown situation in one vocabulary. Nothing catches a failed
 * parse (CLAUDE.md), so a file without markdown must say so, from the three
 * things the `parse-status` error carries (`parseStore`): moving or done;
 * whether a retry could work (`retryable`); and whether the cause is the whole
 * library's (`latching`), which the UI must never blur with "this file is
 * broken". Both flags are optional (a previous session's failure is just
 * `error` in the DB), so unknown is its own case.
 */
type ParseStateKind =
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
  title: string;
  /** The backend's own sentence whenever it gave one. */
  detail: string;
  /** Only a genuine failure is `bad`; "not parsed yet" is not one. */
  tone: "quiet" | "progress" | "good" | "bad" | "hold";
  /** Settings → Library can fix it (a missing or rejected token). */
  fixInSettings: boolean;
}

/** Only shown where `useQualitySweep` will actually retry: not under a latch,
 *  not for a non-retryable failure. */
export const PARSE_SWEEP_NOTE = "Oculus retries outstanding files in the background.";

/**
 * Is this latching cause a token fixable in Settings → Library? (A spent quota
 * or a version mismatch is not.) Falls back to the message when `kind` is
 * absent: only the credential messages name the token.
 */
export function tokenish(kind: string | undefined, message: string): boolean {
  if (kind) return /credential|token/i.test(kind);
  return /token/i.test(message);
}

/** `status` is the live/DB word; `failure` is set only if this session saw
 *  the file fail; `latch` is the app-wide condition in force. */
export function parseStateOf(
  status: string | undefined,
  failure: ParseFailure | undefined,
  latch: ParseLatch | null,
): ParseState {
  // Done and moving beat any failure; the store clears it when a file moves.
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
        // No PARSE_SWEEP_NOTE: the sweep never re-kicks this file.
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

  // `error` from a previous session: unknown retryability is still swept.
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

  // Nothing wrong with this file; parsing is down for the whole library.
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
