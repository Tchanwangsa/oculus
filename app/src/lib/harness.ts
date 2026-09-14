/**
 * The CLI-agent harness, frontend side: types that mirror the Rust
 * `harness` module, the reads over its tables, and the commands.
 *
 * Rust writes every row (`app/src-tauri/src/harness/store.rs`) as the
 * provider's events arrive, and forwards the same events on `harness-event`;
 * this module reads the rows back and `stores/harnessStore.ts` folds the live
 * events into what is on screen. Nothing here talks to a provider.
 */
import { invoke } from "@tauri-apps/api/core";
import { getDb, getSetting } from "@/lib/db";

export type Provider = "claude" | "codex";

export const PROVIDERS: { id: Provider; label: string }[] = [
  { id: "claude", label: "Claude Code" },
  { id: "codex", label: "Codex" },
];

/** The reasoning levels a turn may ask for. `claude --effort` takes exactly
 *  these; Codex declares its own per model (`CodexModel.reasoningEfforts`),
 *  which is why the picker reads the level list off the *model*, not the
 *  provider. `null` is the absence of a level — the flag is left off and the
 *  agent's own default applies. Labels are bb's. */
export const REASONING_LABELS: Record<string, string> = {
  none: "None",
  minimal: "Minimal",
  low: "Low",
  medium: "Medium",
  high: "High",
  xhigh: "Extra High",
  max: "Max",
};

export function reasoningLabel(level: string): string {
  return REASONING_LABELS[level] ?? level;
}

/** One row of the model picker: the id the CLI is given, the name a person
 *  reads, and which reasoning levels that model accepts. */
export interface HarnessModel {
  /** Passed verbatim to `claude --model` or Codex's `model` field. */
  id: string;
  /** The model's own name — never a bare alias. The vendor mark beside it
   *  already says whose it is, so the brand prefix is dropped, the way bb
   *  drops it (`stripModelBrandPrefix`). */
  label: string;
  description: string;
  reasoningEfforts: string[];
  defaultReasoningEffort: string | null;
  /** Where the composer starts before anyone has picked. Not a "default
   *  model" the user can select — every turn names its model outright. */
  isDefault?: boolean;
}

/** The model and level a fresh composer opens on. Nothing is ever sent
 *  without both, so this is a starting selection rather than a fallback the
 *  agent resolves for itself. */
export function defaultSelection(models: HarnessModel[]): {
  model: string | null;
  reasoning: string | null;
} {
  const m = models.find((x) => x.isDefault) ?? models[0];
  if (!m) return { model: null, reasoning: null };
  return { model: m.id, reasoning: m.defaultReasoningEffort ?? m.reasoningEfforts[0] ?? null };
}

/** Every level `claude --effort` accepts. */
const CLAUDE_EFFORTS = ["low", "medium", "high", "xhigh", "max"];

/**
 * Claude Code has no model-list call over the CLI's stream-json protocol —
 * bb probes it through the Agent SDK instead — so this mirrors bb's active
 * catalogue (`plugins/provider-claude-code/src/model-catalog-data.ts`).
 * `--model` also accepts these full names directly, so the ids are the real
 * model names rather than the moving `opus`/`sonnet` aliases; a name that
 * outlives this list still works because it is passed through untouched.
 */
export const CLAUDE_MODELS: HarnessModel[] = [
  {
    id: "claude-fable-5-1",
    label: "Fable 5.1",
    description: "Fable 5.1 for demanding reasoning",
    reasoningEfforts: CLAUDE_EFFORTS,
    defaultReasoningEffort: "high",
  },
  {
    id: "claude-opus-5[1m]",
    label: "Opus 5 (1M)",
    description: "Opus 5 with 1M context for long, complex sessions",
    reasoningEfforts: CLAUDE_EFFORTS,
    defaultReasoningEffort: "high",
    isDefault: true,
  },
  {
    id: "claude-opus-5",
    label: "Opus 5",
    description: "Opus 5 for complex work",
    reasoningEfforts: CLAUDE_EFFORTS,
    defaultReasoningEffort: "high",
  },
  {
    id: "claude-sonnet-5",
    label: "Sonnet 5",
    description: "Sonnet 5 for everyday tasks with deeper reasoning",
    reasoningEfforts: CLAUDE_EFFORTS,
    defaultReasoningEffort: "medium",
  },
  {
    id: "claude-haiku-4-5",
    label: "Haiku 4.5",
    description: "Haiku 4.5 for quick answers",
    reasoningEfforts: ["low"],
    defaultReasoningEffort: "low",
  },
];

export type ToolKind =
  | "read" | "edit" | "write" | "bash" | "search" | "oculus_cli"
  | "task" | "web" | "plan" | "other";

export interface RateWindow {
  label: string;
  used_percent: number;
  resets_at: number | null;
}

/** `HarnessEvent` in `app/src-tauri/src/harness/event.rs`, serde-tagged on `type`. */
export type HarnessEvent =
  | { type: "session_started"; provider_session_id: string; model: string | null; cwd: string }
  | { type: "user_message"; text: string }
  | { type: "turn_started" }
  | { type: "assistant_delta"; text: string }
  | { type: "thinking_delta"; text: string }
  | { type: "assistant_message"; text: string }
  | { type: "thinking"; text: string }
  | { type: "tool_started"; id: string; kind: ToolKind; name: string; title: string; input: unknown }
  | { type: "tool_output_delta"; id: string; text: string }
  | { type: "tool_finished"; id: string; ok: boolean; output: string }
  | { type: "usage"; input_tokens: number; output_tokens: number; context_tokens: number | null; context_window: number | null; cost_usd: number | null }
  | { type: "rate_limits"; windows: RateWindow[] }
  | { type: "thread_titled"; title: string }
  /** A message waiting behind the running turn. Also how an edit to one
   *  arrives: the same `id`, new text. */
  | { type: "queued"; id: string; text: string }
  /** One left the queue — cancelled, cleared by stop, or going out now, in
   *  which case its `user_message` follows. */
  | { type: "unqueued"; id: string }
  /** The provider's handle for the turn that just went out, kept on the
   *  question's row so a later rewind can name it. Nothing on screen changes;
   *  Rust writes it and the webview ignores it. */
  | { type: "turn_anchor"; anchor: string }
  /** Rows from `from_item_id` on are gone: a question was edited or taken
   *  back. `context` is whether the agent was rewound with them; false means
   *  it still holds the original, and the timeline says so. */
  | { type: "rewound"; from_item_id: number; context: boolean }
  | { type: "turn_finished"; status: "completed" | "interrupted" | "failed" }
  | { type: "error"; message: string }
  | { type: "exited"; code: number | null };

export interface HarnessEnvelope {
  threadId: number;
  /** The thread's provider — or, for an account-scoped event with no thread
   *  behind it (rate limits, which arrive on `threadId` 0), whose account. */
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
  /** The subject the thread is scoped to; null is the general thread. Fixed
   *  when the thread is created — both CLIs bind the appended instructions at
   *  session start, so it cannot change under a live session. */
  subject_id: number | null;
  /** The model's own name for the thread once it has been asked for; until
   *  then, the first line of the first message. */
  title: string | null;
  status: "idle" | "running" | "error";
  /** JSON `ThreadUsage`, or null. */
  usage: string | null;
  created_at: string;
  updated_at: string;
}

/** `interrupted` is the one row with nothing in it: a mark left where a turn
 *  was stopped, so the answer above it reads as cut short rather than given
 *  up on. */
export type ItemKind = "user" | "assistant" | "thinking" | "tool" | "error" | "interrupted";

/** A message typed while a turn was running. It is not in the conversation
 *  yet — Rust holds it in memory and writes no row until it goes out — which
 *  is why it can still be edited or dropped. */
export interface QueuedMessage {
  id: string;
  text: string;
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

export interface CodexModel {
  id: string;
  displayName: string;
  description: string;
  reasoningEfforts: string[];
  defaultReasoningEffort: string | null;
  isDefault: boolean;
}

/** Codex reports its own catalogue over `model/list`; this is the same shape
 *  as `CLAUDE_MODELS` so the picker renders both without a special case. */
export function codexAsModels(models: CodexModel[]): HarnessModel[] {
  return models.map((m) => ({
    id: m.id,
    label: m.displayName,
    description: m.description,
    reasoningEfforts: m.reasoningEfforts,
    defaultReasoningEffort: m.defaultReasoningEffort,
    isDefault: m.isDefault,
  }));
}

export function parseUsage(t: HarnessThread | null): ThreadUsage | null {
  if (!t?.usage) return null;
  try {
    return JSON.parse(t.usage);
  } catch {
    return null;
  }
}

/** Parsed `meta` per row object. A tool's output can be hundreds of
 *  kilobytes and every render of its row asks for it again; the row object is
 *  replaced whenever the row changes (the store never mutates one in place),
 *  so keying the cache on it is both cheap and self-invalidating. */
const TOOL_META = new WeakMap<HarnessItem, ToolMeta>();

export function parseToolMeta(item: HarnessItem): ToolMeta {
  if (!item.meta) return {};
  const hit = TOOL_META.get(item);
  if (hit) return hit;
  let meta: ToolMeta = {};
  try {
    meta = JSON.parse(item.meta);
  } catch {
    meta = {};
  }
  TOOL_META.set(item, meta);
  return meta;
}

// ── Reads ────────────────────────────────────────────────────────────────────

export async function getHarnessThreads(limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(
    `SELECT * FROM harness_threads ORDER BY updated_at DESC, id DESC LIMIT $1`,
    [limit],
  );
}

export async function getHarnessItems(threadId: number): Promise<HarnessItem[]> {
  const db = await getDb();
  return db.select<HarnessItem[]>(
    `SELECT * FROM harness_items WHERE thread_id = $1 ORDER BY id ASC`,
    [threadId],
  );
}

export async function getHarnessRateLimits(provider: Provider): Promise<RateWindow[]> {
  const raw = await getSetting(`harness_rate_limits_${provider}`);
  if (!raw) return [];
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

// ── Commands ─────────────────────────────────────────────────────────────────

export interface SendOptions {
  model?: string | null;
  reasoningEffort?: string | null;
  /** Only read when the send creates the thread; an open thread keeps its
   *  own scope. */
  subjectId?: number | null;
}

export function harnessSend(
  threadId: number | null,
  provider: Provider,
  text: string,
  options: SendOptions,
): Promise<number> {
  return invoke<number>("harness_send", { threadId, provider, text, options });
}

/**
 * Ask the same question differently. The thread is rewound to that question —
 * it and everything after it stop being rows — and the new text goes as the
 * next turn.
 *
 * The agent is rewound too, over its own control channel, so its context
 * matches what is on screen. The exception is a question asked before the
 * anchor was recorded, or one whose session the CLI has since dropped: the
 * rows still go, and the `rewound` event's `context: false` is what draws the
 * note saying the agent kept the original.
 */
export function harnessEditResend(
  threadId: number,
  itemId: number,
  text: string,
  options: SendOptions,
): Promise<void> {
  return invoke("harness_edit_resend", { threadId, itemId, text, options });
}

/**
 * Take the thread back to just before a question: it and everything after it
 * stop being rows, and the question comes back as text for the composer.
 * Claude Code's rewind without the branching — there is one thread, so going
 * back means the rest is gone. The agent is rewound too, on the same terms as
 * `harnessEditResend`; not sending is the whole difference between them.
 */
export function harnessRewind(threadId: number, itemId: number): Promise<string> {
  return invoke<string>("harness_rewind", { threadId, itemId });
}

/** What is still waiting behind this thread's turn. The queue lives in Rust's
 *  memory rather than the database — a message that was never sent is not
 *  history — so a reloaded page asks for it. */
export function harnessQueued(threadId: number): Promise<QueuedMessage[]> {
  return invoke<QueuedMessage[]>("harness_queued", { threadId });
}

export function harnessUnqueue(threadId: number, queueId: string): Promise<void> {
  return invoke("harness_unqueue", { threadId, queueId });
}

export function harnessEditQueued(threadId: number, queueId: string, text: string): Promise<void> {
  return invoke("harness_edit_queued", { threadId, queueId, text });
}

/** Stop the running turn and drop whatever was waiting behind it. The dropped
 *  messages come back so the composer can hand them to the student rather
 *  than swallow what they typed. */
export function harnessInterrupt(threadId: number): Promise<string[]> {
  return invoke<string[]>("harness_interrupt", { threadId });
}

export function harnessDeleteThread(threadId: number): Promise<void> {
  return invoke("harness_delete_thread", { threadId });
}

export function harnessHealth(): Promise<BridgeHealth[]> {
  return invoke<BridgeHealth[]>("harness_health");
}

/** Ask the provider for its plan windows now, rather than waiting for a turn
 *  to report them. The answer comes back as a `rate_limits` event like any
 *  other, so nothing here reads a return value. Codex answers; Claude has no
 *  such request and ignores it. */
export function harnessRefreshRateLimits(provider: Provider): Promise<void> {
  return invoke<void>("harness_refresh_rate_limits", { provider });
}

export function harnessCodexModels(): Promise<CodexModel[]> {
  return invoke<CodexModel[]>("harness_codex_models");
}
