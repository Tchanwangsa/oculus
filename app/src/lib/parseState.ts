import type { ParseFailure, ParseLatch } from "@/stores/parseStore";

/**
 * A file's markdown situation in one vocabulary. Nothing catches a failed
 * parse (docs/parsing.md), so a file without markdown must say so, from the three
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
  | "skipped"
  | "unparsed"
  | "failed"
  | "permanent"
  | "blocked";

export interface ParseState {
  kind: ParseStateKind;
  /** One word for the state; a file row's icon is labelled with it. */
  label: string;
  title: string;
  /** The backend's own sentence, or empty: the title says the rest. */
  detail: string;
  /** A few words for a file row's tooltip; `detail` is shown on the file. */
  summary: string;
  /** Only a genuine failure is `bad`; "not parsed yet" and a skip are not. */
  tone: "quiet" | "progress" | "good" | "bad" | "hold";
  /** Settings → Parsing can fix it (a missing or rejected token). */
  fixInSettings: boolean;
}

/** Only shown where `useQualitySweep` will actually retry: not under a latch,
 *  not for a non-retryable failure. */
export const PARSE_SWEEP_NOTE = "Oculus retries outstanding files in the background.";

/**
 * Is this latching cause a token fixable in Settings → Parsing? (A spent quota
 * or a version mismatch is not.) Falls back to the message when `kind` is
 * absent: only the credential messages name the token.
 */
export function tokenish(kind: string | undefined, message: string): boolean {
  if (kind) return /credential|token/i.test(kind);
  return /token/i.test(message);
}

/** The row tooltip's line for a failure, by `ParseError::kind` in
 *  `parse/mod.rs`. Empty when the kind is unknown (a previous session's). */
export function summaryOf(kind: string | undefined): string {
  switch (kind) {
    case "offline": return "Couldn't reach MinerU.";
    case "io": return "Couldn't save the parsed output.";
    case "not_ready": return "The parser isn't ready yet.";
    case "document": return "MinerU couldn't read this PDF.";
    case "too_large": return "Too large for MinerU.";
    case "quota_exhausted": return "Daily quota used up. Resumes on its own.";
    case "missing_credentials": return "No MinerU token saved.";
    case "rejected_credentials": return "MinerU rejected the token.";
    case "unreadable_credentials": return "The keychain refused the MinerU token.";
    case "credential_broker": return "oculus-keyd couldn't send the request.";
    case "version_mismatch": return "Parser version mismatch.";
    default: return "";
  }
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
      detail: "",
      summary: "",
      tone: "good",
      fixInSettings: false,
    };
  }
  if (status === "running") {
    return {
      kind: "running",
      label: "parsing",
      title: "Parsing",
      detail: "",
      summary: "",
      tone: "progress",
      fixInSettings: false,
    };
  }
  if (status === "queued") {
    return {
      kind: "queued",
      label: "queued",
      title: "Queued",
      detail: "",
      summary: "",
      tone: "progress",
      fixInSettings: false,
    };
  }
  // The user's choice: calm, and beats a latch, since nothing is waiting on it.
  if (status === "skipped") {
    return {
      kind: "skipped",
      label: "skipped",
      title: "Skipped",
      detail: "",
      summary: "Not parsed until you ask.",
      tone: "quiet",
      fixInSettings: false,
    };
  }

  if (failure) {
    if (failure.latching) {
      return {
        kind: "blocked",
        label: "on hold",
        title: "Parsing unavailable",
        detail: failure.message,
        summary: summaryOf(failure.kind),
        tone: "hold",
        fixInSettings: tokenish(failure.kind, failure.message),
      };
    }
    if (failure.retryable === false) {
      return {
        kind: "permanent",
        label: "can't parse",
        title: "Can't parse this file",
        detail: failure.message,
        summary: summaryOf(failure.kind),
        tone: "bad",
        fixInSettings: false,
      };
    }
    return {
      kind: "failed",
      label: "failed",
      title: "Parse failed",
      detail: failure.message,
      summary: summaryOf(failure.kind),
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
      detail: "",
      summary: "",
      tone: "bad",
      fixInSettings: false,
    };
  }

  // Nothing wrong with this file; parsing is down for the whole library.
  if (latch) {
    return {
      kind: "blocked",
      label: "on hold",
      title: "Parsing unavailable",
      detail: latch.message,
      summary: summaryOf(latch.kind),
      tone: "hold",
      fixInSettings: tokenish(latch.kind, latch.message),
    };
  }

  return {
    kind: "unparsed",
    label: "not parsed",
    title: "Not parsed yet",
    detail: "",
    summary: "",
    tone: "quiet",
    fixInSettings: false,
  };
}

/** Tailwind colour for a state's icon. */
export const PARSE_TONE_CLASS: Record<ParseState["tone"], string> = {
  quiet: "text-muted-foreground/70",
  progress: "text-brand",
  good: "text-success",
  bad: "text-destructive",
  hold: "text-warning",
};
