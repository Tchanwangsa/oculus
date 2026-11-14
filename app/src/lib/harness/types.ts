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
  | { type: "usage"; input_tokens: number; output_tokens: number; context_tokens: number | null; context_window: number | null; cost_usd: number | null }
  | { type: "rate_limits"; windows: RateWindow[] }
  | { type: "thread_titled"; title: string }
  /** The student's messages waiting their turn, whole and in order, told
   *  again whenever the queue changes. */
  | { type: "queue"; messages: QueuedMessage[] }
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
  provider: Provider;
  provider_session_id: string | null;
  model: string | null;
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
  | "compaction";

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

/** `harness_runs.execution_status`. */
export type RunStatus = "preparing" | "running" | "completed" | "interrupted" | "failed";

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
}

export interface BridgeHealth {
  provider: Provider;
  label: string;
  path: string | null;
  version: string | null;
  error: string | null;
  overrideEnv: string;
}
