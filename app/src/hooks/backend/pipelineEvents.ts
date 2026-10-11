import { setParseStatus } from "@/lib/db";
import { usePipelineStore } from "@/stores/sync/pipelineStore";
import { createParseStatusWriter } from "@/lib/pipeline/parseStatusWriter";

/** `parse-status` and `files.parse_status` share this vocabulary; `"quality"`
 *  is a finished parse, and renaming it would invalidate every stored row.
 *  `"skipped"` is the user's choice, kept until they parse the file. */
export const PARSE_STATUSES = new Set(["queued", "running", "quality", "error", "skipped"]);
export const parseStatuses = createParseStatusWriter(setParseStatus);

export const EMBED_STATUSES = new Set(["queued", "running", "done", "error"]);

/** `embed-status`, exactly as `app/src-tauri/src/embed/events.rs` emits it. */
export interface EmbedJob {
  relative_path: string;
  subject_id: number;
  status: string;
  pages_done?: number;
  total_pages?: number;
  error?: string;
  kind?: string;
  retryable?: boolean;
  latching?: boolean;
  /** On `running` only, while a rate limit holds the file. */
  waiting_until_ms?: number;
  waiting_reason?: string;
}

/** Every embed event that is not a wait ends one. */
export const NO_WAIT = { embedWaitingUntil: undefined, embedWaitingReason: undefined } as const;

export const pipeline = () => usePipelineStore.getState();

export const NO_ERROR = {
  error: undefined,
  errorKind: undefined,
  errorRetryable: undefined,
  errorLatching: undefined,
} as const;

/** The row has one error slot: a recovering stage clears it only while the
 *  other stage is not still failed. */
export const clearErrorUnless = (path: string, other: "parse" | "embed") =>
  pipeline().items[path]?.[other] === "error" ? {} : NO_ERROR;

/** An embed implies a finished parse only on first sighting; a re-parse
 *  queued behind an old embed keeps its own state. */
export const parseIfUnseen = (path: string) =>
  (pipeline().items[path]?.parse ?? "pending") === "pending" ? { parse: "done" as const } : {};
