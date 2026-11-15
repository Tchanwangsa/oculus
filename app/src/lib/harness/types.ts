import type { Provider } from "./providers";
import type { ToolKind } from "./meta";
import type { Proposal } from "./proposals";

/** Whether a provider's CLI is available to model pickers. */
export type ProviderHealth = "unknown" | "installed" | "missing";

export interface RateWindow {
  label: string;
  used_percent: number;
  resets_at: number | null;
}

/** `HarnessEvent` in `app/src-tauri/src/harness/event/mod.rs`, serde-tagged on `type`. */
type HarnessEvent =
  | { type: "proposal"; proposal: Proposal }
  | { type: "session_started"; provider_session_id: string; model: string | null; cwd: string }
  /** `at`: playhead second for a message sent from the lecture dock. */
  | { type: "user_message"; text: string; at?: number | null }
  /** A turn the agent started itself (Claude picking up a finished
   *  background task); `after` names those tasks. Holds the thread until its
   *  `turn_finished`, like a sent message. */
  | { type: "unprompted_turn"; after: string[] }
  /** A run Oculus started after a card settled, in place of a question
   *  (docs/harness.md, "A card can continue the agent"). `text` is its
   *  timeline line; `proposal` the card's id. Holds the thread like a send. */
  | { type: "continued"; text: string; proposal: string }
  | { type: "turn_started" }
  | { type: "assistant_delta"; text: string }
  | { type: "thinking_delta"; text: string }
  | { type: "assistant_message"; text: string }
  | { type: "thinking"; text: string; ms?: number | null }
  | { type: "tool_started"; id: string; kind: ToolKind; name: string; title: string; input: unknown }
  | { type: "tool_output_delta"; id: string; text: string }
  | { type: "tool_finished"; id: string; ok: boolean; output: string; title?: string | null }
  /** The CLI's reviewer decided on a call in Auto mode; Rust stored it on
   *  the call's row (`env.itemId`) as `meta.review`. */
  | { type: "tool_reviewed"; id: string; review: ToolReview }
  | { type: "usage"; input_tokens: number; output_tokens: number; context_tokens: number | null; context_window: number | null; cost_usd: number | null }
  | { type: "rate_limits"; windows: RateWindow[] }
  | { type: "thread_titled"; title: string }
  /** The student's messages waiting their turn, whole and in order, told
   *  again whenever the queue changes. */
  | { type: "queue"; messages: QueuedMessage[] }
  /** The student asked this run to stop; it ends on its own `turn_finished`. */
  | { type: "stop_requested"; run_id: number }
  /** Fifteen seconds after the stop the run is still open: its CLI is not
   *  answering, and Force stop is offered. `others` is how many other chats
   *  have a run on the same server, which a force stop would end too. */
  | { type: "stop_unresponsive"; run_id: number; others: number }
  /** The provider's handle for a later rewind; the webview ignores it. */
  | { type: "turn_anchor"; anchor: string }
  /** Rows from `from_item_id` on are gone; `context` is whether the agent
   *  was rewound too. */
  | { type: "rewound"; from_item_id: number; context: boolean }
  /** Antigravity only: a refusal that already ended the turn. `rule` would
   *  allow it, for `harnessAntigravityAllow`. */
  | {
      type: "permission_needed";
      tool: string;
      action: string;
      target: string | null;
      rule: string | null;
    }
  /** Rust's settlement of a student's run: its one answer row (none when no
   *  text arrived), the earlier replies that are now work, how it ended and
   *  its stored interval (SQLite UTC). */
  | {
      type: "run_settled";
      answer_item_id: number | null;
      work_item_ids: number[];
      delivery: string;
      status?: RunStatus;
      started_at?: string | null;
      ended_at?: string | null;
    }
  /** The chat's selection changed (`harnessSelectModel`, or a send under
   *  another one): what its next send will use. Runs keep their own. */
  | { type: "selected"; provider: Provider; model: string | null; effort: string | null }
  /** A run's context handoff, told when the run is written and again when
   *  its start step is over (`RunHandoff`). */
  | ({ type: "handoff"; run_id: number } & RunHandoff)
  /** What the run was sent with (the chat's selection when it started),
   *  told once after its question. */
  | { type: "run_selected"; provider: Provider; model: string | null; effort: string | null }
  /** A request saved with its row (`env.itemId`), and every later change
   *  to it: answered, its answer's delivery learned, or ended unanswered. */
  /** `waiting`: the thread's requests waiting on the student once this one
   *  is saved. */
  | { type: "request_opened"; request: HarnessRequest; waiting: number }
  | { type: "request_updated"; request: HarnessRequest; waiting: number }
  /** How the chat asks from its next run on (`harnessSetPermissionMode`). */
  | { type: "permission_mode"; mode: PermissionMode }
  /** The row saying an answer, or part of it, did not arrive. */
  | { type: "delivery_notice"; text: string }
  | { type: "compaction"; source?: string }
  /** Stored evidence of missing text; the timeline shows `delivery_notice` instead. */
  | { type: "delivery_gap"; source?: string; cause: string }
  | { type: "turn_finished"; status: "completed" | "interrupted" | "failed" }
  /** `auth` names the provider when the CLI has no usable credentials; the
   *  timeline draws a sign-in card instead of an error. */
  | { type: "error"; message: string; auth: Provider | null; limit?: boolean }
  | { type: "exited"; code: number | null };

export interface HarnessEnvelope {
  threadId: number;
  /** For account-scoped events on `threadId` 0 (rate limits), whose account. */
  provider: Provider;
  itemId: number | null;
  /** The student run this event belongs to; absent for an unprompted turn
   *  and for account-scoped events. */
  runId?: number;
  attemptId?: number;
  entityId?: string;
  /** Increases with every envelope; a repeat is ignored. */
  sequence?: number;
  /** The thread's revision when Rust emitted this: everything it tells of
   *  was saved at or below it, so a snapshot at that revision or later
   *  (`harnessLoad`) already holds its rows. */
  revision?: number;
  event: HarnessEvent;
}

export interface ThreadUsage {
  inputTokens: number;
  outputTokens: number;
  contextTokens: number | null;
  contextWindow: number | null;
  costUsd: number | null;
}

export interface HarnessThread {
  id: number;
  /** `provider`, `model` and `effort` are the thread's selection: what its
   *  next send will use. What an answer came from is on its run. */
  provider: Provider;
  provider_session_id: string | null;
  model: string | null;
  effort?: string | null;
  /** Null is the general thread. Fixed at creation: CLIs bind instructions
   *  at session start. */
  subject_id: number | null;
  /** Set for a lecture-dock thread; it also decides `subject_id`. */
  lecture_id: string | null;
  /** The model's title once asked for; until then the first message's line. */
  title: string | null;
  status: "idle" | "running" | "error";
  /** JSON `ThreadUsage`, or null. */
  usage: string | null;
  created_at: string;
  updated_at: string;
  /** When the student last had it on screen (`harness_thread_viewed`). */
  viewed_at?: string | null;
  /** When its latest run ended: `MAX(harness_runs.ended_at)`, read, never stored. */
  settled_at?: string | null;
  /** How the chat asks for what its sandbox blocks; `ask` when absent. */
  permission_mode?: PermissionMode;
  /** Its requests waiting on the student: read with the list (`reads.ts`),
   *  then the `waiting` of each request event. The sidebar's Approval. */
  pending_requests?: number;
}

/** `ask` shows each request; `quiet` ("Don't ask") declines without one;
 *  `auto` hands it to the CLI's reviewer, which asks as `ask` where the
 *  bridge has none (`Capabilities.autoReview`). */
export type PermissionMode = "ask" | "quiet" | "auto";

/** One answer a request offers (`RequestOption` in
 *  `app/src-tauri/src/harness/requests/mod.rs`). `scope` says, in a
 *  sentence, how far it reaches past this one call. */
export interface RequestOption {
  id: string;
  label: string;
  decision: "accept" | "accept_for_session" | "decline";
  scope?: string;
}

/** What the agent asked mid-turn (`RequestView` in
 *  `app/src-tauri/src/harness/requests/mod.rs`). Answer a `pending` one with
 *  `harnessRespond`; `responder` other than `live` cannot be answered and
 *  `reason` says why. `delivery` is what is known of the answer's way to
 *  the CLI: `uncertain` was never resent, `failed` may be answered again. */
export interface HarnessRequest {
  id: number;
  thread_id: number;
  run_id: number | null;
  /** The `request` row that shows it. */
  item_id: number | null;
  provider: Provider;
  kind: "approval" | "question";
  action: "command" | "file_change" | "permissions" | "network" | "access" | "tool" | "rule";
  /** Per action. A command: `command`, `cwd`, `reason`, `outsideSandbox`.
   *  A file change: `changes` (`path`, `diff`, `kind`), `reason`. Permissions:
   *  `permissions` (the profile asked for), `reason`. Any: `agent` (`path`,
   *  `thread`) when a child agent asked, `tool` (the native item id, which
   *  its tool row's `ref_id` matches). */
  payload: Record<string, unknown>;
  options: RequestOption[];
  status: "pending" | "resolved" | "expired" | "cancelled";
  responder: "live" | "message" | "not_resumable";
  reason: string | null;
  /** The option chosen. */
  response: string | null;
  delivery: "none" | "sending" | "acknowledged" | "failed" | "uncertain";
  created_at: string;
  resolved_at: string | null;
}

/** `interrupted` is an empty marker where a turn was stopped. */
type ItemKind =
  | "user"
  | "assistant"
  | "thinking"
  | "tool"
  | "error"
  | "interrupted"
  /** `meta` is `PermissionMeta`. */
  | "permission"
  /** Marks a turn the agent started itself; `meta` is `{ after: string[] }`. */
  | "unprompted"
  /** Heads a card's continuation run; `content` is its line ("Sync finished —
   *  continuing"), `meta` is `{ proposal: string }`. Never the student's words. */
  | "continuation"
  /** A run whose answer, or part of it, never arrived; `content` says which. */
  | "delivery_notice"
  | "compaction"
  /** Something the agent asked the student; `ref_id` is the `HarnessRequest`'s
   *  id and the row holds nothing else. */
  | "request";

/** A message waiting its turn: a queued run in the database
 *  (`harness/store/queue.rs`), so it survives a reload and a restart. */
export interface QueuedMessage {
  /** Its run's id. */
  id: string;
  text: string;
  /** The queue is held: nothing goes out until the student resumes it. */
  held: boolean;
}

export interface ErrorMeta {
  auth?: Provider;
  /** The provider's usage limit is spent (`is_usage_limit` in Rust). */
  limit?: boolean;
}

export interface ToolMeta {
  kind?: ToolKind;
  name?: string;
  input?: unknown;
  ok?: boolean | null;
  output?: string | null;
  /** The CLI's own reviewer on this call, in the chat's Auto mode: the
   *  student was not asked (`tool_reviewed`). */
  review?: ToolReview;
}

/** A CLI reviewer's decision on one tool call (`Review` in
 *  `app/src-tauri/src/harness/event/mod.rs`). A `denied` call did not run. */
export interface ToolReview {
  status: "in_progress" | "approved" | "denied" | "timed_out" | "aborted";
  risk?: "low" | "medium" | "high" | string;
  rationale?: string;
  /** Who decided, as the CLI names it. */
  source?: string;
}

export interface HarnessItem {
  id: number;
  thread_id: number;
  kind: ItemKind;
  ref_id: string | null;
  content: string | null;
  /** JSON `ToolMeta` for tools. */
  meta: string | null;
  created_at: string;
  /** Set when Rust settles a run: its one `answer`, earlier replies `commentary`. */
  display_role?: "answer" | "commentary" | null;
  /** The student run that wrote it; null for an unprompted turn and for rows
   *  from before runs existed. */
  run_id?: number | null;
}

/** The earlier conversation a run's first prompt carried to a session that
 *  had not seen it (`HandoffView` in `harness/handoff/store.rs`). */
export interface RunHandoff {
  /** `full`: the whole chat, to a provider new to it or a session that had
   *  to start over. `delta`: only the runs a returning provider missed. */
  kind: "full" | "delta";
  /** `pending` until the provider has the prompt, then `inline`; `failed`
   *  was never delivered (it did not fit, or the send never went out). */
  status: "pending" | "inline" | "failed";
  /** The provider that answered before, when it is another one; null for
   *  a handoff within one provider. */
  from: Provider | null;
  /** How many earlier messages were left out to fit. */
  omitted: number;
}

/** `harness_runs.execution_status`. */
export type RunStatus = "preparing" | "starting" | "running" | "completed" | "interrupted" | "failed";

/** What the timeline needs of a `harness_runs` row: how it ended, its answer
 *  and its stored wall-clock interval. */
export interface HarnessRun {
  id: number;
  status: RunStatus;
  delivery: string;
  answer_item_id: number | null;
  /** Epoch ms; null when unknown. */
  started_ms: number | null;
  ended_ms: number | null;
  /** The student asked it to stop and its turn has not ended yet. */
  stopping?: boolean;
  /** When they asked, epoch ms: a reload finds a stop that is overdue
   *  (`stopOverdue`) without having heard `stop_unresponsive`. */
  stop_requested_ms?: number | null;
  /** Its CLI did not answer the stop (`stop_unresponsive`); `others` other
   *  chats have a run on the same server. */
  unresponsive?: { others: number };
  /** What it was sent with: an answer's footer and a divider between runs
   *  read these, never the thread, whose selection is the next send's.
   *  Absent on a run from before they were recorded. */
  provider?: Provider;
  model?: string | null;
  effort?: string | null;
  /** The model its CLI reported, where that differs from `model`. */
  resolved_model?: string | null;
  /** Its context handoff, when its prompt carried one. */
  handoff?: RunHandoff;
}

export interface BridgeHealth {
  provider: Provider;
  label: string;
  path: string | null;
  version: string | null;
  error: string | null;
  overrideEnv: string;
}
