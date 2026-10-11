import type { Provider } from "./providers";
import type { ToolKind } from "./meta";

/** Whether a provider's CLI is available to model pickers. */
export type ProviderHealth = "unknown" | "installed" | "missing";

export interface RateWindow {
  label: string;
  used_percent: number;
  resets_at: number | null;
}

/** `HarnessEvent` in `app/src-tauri/src/harness/event/mod.rs`, serde-tagged on `type`. */
type HarnessEvent =
  | { type: "session_started"; provider_session_id: string; model: string | null; cwd: string }
  /** `at`: playhead second for a message sent from the lecture dock. */
  | { type: "user_message"; text: string; at?: number | null }
  | { type: "turn_started" }
  | { type: "assistant_delta"; text: string }
  | { type: "thinking_delta"; text: string }
  | { type: "assistant_message"; text: string }
  | { type: "thinking"; text: string }
  | { type: "tool_started"; id: string; kind: ToolKind; name: string; title: string; input: unknown }
  | { type: "tool_output_delta"; id: string; text: string }
  | { type: "tool_finished"; id: string; ok: boolean; output: string; title?: string | null }
  | { type: "usage"; input_tokens: number; output_tokens: number; context_tokens: number | null; context_window: number | null; cost_usd: number | null }
  | { type: "rate_limits"; windows: RateWindow[] }
  | { type: "thread_titled"; title: string }
  /** Queued behind the running turn; an edit re-sends the same `id`. */
  | { type: "queued"; id: string; text: string }
  | { type: "unqueued"; id: string }
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
  | { type: "turn_finished"; status: "completed" | "interrupted" | "failed" }
  /** `auth` names the provider when the CLI has no usable credentials; the
   *  timeline draws a sign-in card instead of an error. */
  | { type: "error"; message: string; auth: Provider | null }
  | { type: "exited"; code: number | null };

export interface HarnessEnvelope {
  threadId: number;
  /** For account-scoped events on `threadId` 0 (rate limits), whose account. */
  provider: Provider;
  itemId: number | null;
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
  | "permission";

/** Held in Rust's memory, not the database, until it goes out. */
export interface QueuedMessage {
  id: string;
  text: string;
}

export interface ErrorMeta {
  auth?: Provider;
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
}

export interface BridgeHealth {
  provider: Provider;
  label: string;
  path: string | null;
  version: string | null;
  error: string | null;
  overrideEnv: string;
}
