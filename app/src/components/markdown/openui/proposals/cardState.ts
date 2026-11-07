import type { ProposalStatus } from "@/lib/harness/proposals";

/** Every proposal and job card is in exactly one of these, and `CardFrame`
 * draws each one the same way for every kind (docs/ui.md). */
export type CardPhase =
  | "streaming"
  | "uncommitted"
  | "loading"
  | "pending"
  | "applying"
  | "created"
  | "failed"
  | "undone"
  | "discarded"
  | "superseded";

/** What failed: the row never loaded, a decision was refused, or a job ended in error. */
export type CardFailure = "load" | "apply" | "job";

export interface CardState {
  phase: CardPhase;
  failure?: CardFailure;
  /** Rows and fields take input only here; every other phase is read-only. */
  editable: boolean;
  /** The body shows but takes no click: not ready yet, or a decision is in
   * flight. A running job is not locked; its body reports progress. */
  locked: boolean;
  /** Discarded and superseded cards keep only their header. */
  collapsed: boolean;
}

export interface CardInputs {
  /** The fence is still arriving (OpenUI's `useIsStreaming`). */
  streaming: boolean;
  /** The proposal id, null until the reply row is committed. */
  id: string | null;
  /** The loaded row's status, undefined while it loads. */
  status?: ProposalStatus;
  busy: boolean;
  error?: string;
  /** A job card's persisted `state.job.phase`. */
  job?: unknown;
}

const make = (phase: CardPhase, failure?: CardFailure, busy = false): CardState => ({
  phase,
  failure,
  locked: phase === "streaming" || phase === "uncommitted" || phase === "loading" || failure === "load" || (phase === "applying" && busy),
  editable: phase === "pending" || (phase === "failed" && failure === "apply"),
  collapsed: phase === "discarded" || phase === "superseded",
});

/** streaming → uncommitted → loading → pending → applying → created / failed
 * → undone, plus discarded and superseded. A refused decision is `failed`
 * but stays editable so the student can fix it and try again. */
export function cardState({ streaming, id, status, busy, error, job }: CardInputs): CardState {
  if (streaming) return make("streaming");
  if (!id) return make("uncommitted");
  if (!status) return error ? make("failed", "load") : make("loading");
  if (status === "discarded" || status === "superseded" || status === "undone") return make(status);
  if (busy) return make("applying", undefined, true);
  if (status === "pending") return error ? make("failed", "apply") : make("pending");
  if (job === "running") return make("applying");
  if (job === "error") return make("failed", "job");
  return make("created");
}
