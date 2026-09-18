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
import { filterOffered, loadCatalogue } from "@/lib/opencodeCatalogue";

export type Provider = "claude" | "codex" | "opencode";

/** The reasoning levels a turn may ask for, **weakest first** — the key order
 *  here is the canonical one, and `sortReasoning` below is the only thing that
 *  decides how a level row reads. `claude --effort` takes exactly these; Codex
 *  declares its own per model (`CodexModel.reasoningEfforts`), which is why
 *  the picker reads the level list off the *model*, not the provider. `null`
 *  is the absence of a level — the flag is left off and the agent's own
 *  default applies. Labels are bb's. */
export const REASONING_LABELS: Record<string, string> = {
  none: "None",
  minimal: "Minimal",
  low: "Low",
  medium: "Medium",
  high: "High",
  xhigh: "Extra High",
  max: "Max",
  ultra: "Ultra",
};

export function reasoningLabel(level: string): string {
  return REASONING_LABELS[level] ?? level;
}

/** Weakest to strongest, from the key order of `REASONING_LABELS` so there is
 *  one table rather than two that can drift. */
const REASONING_ORDER = Object.keys(REASONING_LABELS);

/**
 * A model's levels in the one order a person expects to read them.
 *
 * **No provider hands them over sorted, and one of them hands them over
 * alphabetically.** opencode's catalogue spells `variants` as an object keyed
 * by level id, and a JSON map has no order worth keeping, so the levels
 * arrived as High · Low · Max — which reads like a ranking and is not one.
 * Sorting here rather than at each source covers all three catalogues,
 * including a Codex that adds a level to an existing model tomorrow.
 *
 * A level this build has no name for keeps its place at the end, in the order
 * the provider gave it: an unknown id is already shown verbatim by
 * `reasoningLabel`, and guessing where it ranks would be worse than tacking
 * it on.
 */
export function sortReasoning(levels: string[]): string[] {
  const rank = (l: string) => {
    const i = REASONING_ORDER.indexOf(l);
    return i === -1 ? REASONING_ORDER.length : i;
  };
  return [...levels].sort((a, b) => rank(a) - rank(b));
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
  /** What the provider says this model can do, for opencode's rows only —
   *  Claude's and Codex's lists carry none of these, and absent reads as
   *  capable. `unusableReason` in `@/lib/opencodeCatalogue` is the rule. */
  toolCall?: boolean;
  textInput?: boolean;
  textOutput?: boolean;
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

/** One CLI agent the harness can drive: its id, the name a person reads, and
 *  where its catalogue comes from. */
export interface ProviderInfo {
  id: Provider;
  label: string;
  /** The models this provider has without being asked. Claude Code answers no
   *  model-list call over `stream-json`, so its catalogue is compiled in
   *  above; `null` says the CLI has to be asked, which is what
   *  `fetchModels` below does. Everything that used to test
   *  `provider === "claude"` — the composer's opening selection, what a
   *  provider switch clears — is really asking this question. */
  staticModels: HarnessModel[] | null;
  /** Ask the CLI for its catalogue. Present exactly when `staticModels` is
   *  null, and already adapted to the picker's shape, so a call site never
   *  names a provider to know which command to invoke
   *  (`useProviderModels` in `app/src/hooks/useProviderModels.ts`). */
  fetchModels?: () => Promise<HarnessModel[]>;
  /** How this CLI's own sign-in ends — the one thing `SignInDialog` branches
   *  on, and a property of the provider rather than a test on its id.
   *
   *  `"code"` is Claude: `claude auth login` prints the authorize URL and then
   *  *blocks reading a pasted authorization code off stdin*, so the dialog has
   *  to offer a field. `"callback"` is Codex: `codex login` runs a loopback
   *  server on :1455 and finishes by itself when the browser comes back, so
   *  there is nothing to type and the dialog only waits.
   *
   *  `null` is opencode, and it is a decision rather than a gap. opencode's
   *  credentials are per *provider*, not per CLI, and Settings → AI already
   *  owns that whole surface — the catalogue, the form specs, the OAuth flows
   *  (`OpencodeProvidersSection.tsx`). A second path to the same store would
   *  be a second answer to "am I signed in", so `harness_sign_in_start`
   *  rejects opencode and `harness_sign_in_status` answers `signedIn: null`. */
  signIn: "code" | "callback" | null;
  /** What a picker should say when this provider's list comes back empty and
   *  its CLI *is* installed — the one case "No models available" is a dead end
   *  rather than a fact, because there is something the student can do about
   *  it.
   *
   *  opencode's list is filtered by the catalogue below, so an installed,
   *  connected opencode with nothing probed yet legitimately has zero rows,
   *  and the sentence has to point at where the probing happens. Claude and
   *  Codex leave this unset: an empty catalogue from either of them is an
   *  answer their CLI gave, not a step that was skipped.
   *
   *  It is a field here rather than an `id === "opencode"` in the picker for
   *  the same reason `staticModels`, `signIn` and `health` are: this file is
   *  the one place a provider is declared, and the picker stays provider-blind
   *  so a fourth agent costs it nothing. */
  emptyNote?: string;
}

/**
 * Every provider, in the order the picker lists them. **The one place a
 * provider is declared**: the marks (`ProviderMark.tsx`), the picker's rows,
 * the job registry's read in `db.ts` and the composer's selection all come off
 * this list rather than off a hardcoded pair, so adding a fourth agent is an
 * entry here plus its mark.
 *
 * opencode brands itself lowercase, so the label is not title-cased.
 */
export const PROVIDERS: ProviderInfo[] = [
  { id: "claude", label: "Claude Code", staticModels: CLAUDE_MODELS, signIn: "code" },
  {
    id: "codex",
    label: "Codex",
    staticModels: null,
    fetchModels: () => harnessCodexModels().then(codexAsModels),
    signIn: "callback",
  },
  {
    id: "opencode",
    label: "opencode",
    staticModels: null,
    // opencode is the one provider whose catalogue is bigger than its truth:
    // 218 providers' worth of rows, some of which answer 400 or 401 when
    // asked (`app/src/lib/opencodeCatalogue.ts`). The filter lives inside the
    // entry so `useProviderModels` and `ModelPicker` still name no provider —
    // "fetch the list" and "fetch the list that works" are the same job from
    // where they stand.
    fetchModels: async () => {
      const models = opencodeAsModels(await harnessOpencodeModels());
      return filterOffered(models, await loadCatalogue());
    },
    signIn: null,
    emptyNote: "Sign in to a provider in Settings → AI to get models here.",
  },
];

export function providerInfo(provider: Provider): ProviderInfo | undefined {
  return PROVIDERS.find((p) => p.id === provider);
}

export function providerLabel(provider: Provider): string {
  return providerInfo(provider)?.label ?? provider;
}

/** Which sign-in shape this agent has, or null for one that has none here.
 *  Read off `PROVIDERS` rather than tested on an id, so a fourth agent costs
 *  the dialog nothing. */
export function signInFlow(provider: Provider): "code" | "callback" | null {
  return providerInfo(provider)?.signIn ?? null;
}

/** Narrow a string that came out of the database — or out of an older build —
 *  to a provider. Driven off `PROVIDERS` so it can never go stale against the
 *  union: a stored row naming a provider this build has is kept, and one
 *  naming a provider it does not is dropped. */
export function isProvider(value: unknown): value is Provider {
  return typeof value === "string" && PROVIDERS.some((p) => p.id === value);
}

/** The selection a fresh composer opens on for one provider. A provider whose
 *  catalogue is fetched has nothing to open on until its CLI answers — the
 *  picker fills it in the moment the list lands — so the answer there is an
 *  empty selection rather than a guess. */
export function defaultSelectionFor(provider: Provider): {
  model: string | null;
  reasoning: string | null;
} {
  const models = providerInfo(provider)?.staticModels;
  return models ? defaultSelection(models) : { model: null, reasoning: null };
}

export type ToolKind =
  | "read" | "edit" | "write" | "bash" | "search" | "oculus_cli"
  | "task" | "web" | "plan" | "other";

/**
 * "Ran", "Read", "Edited" — past tense once done, present while running.
 *
 * Here rather than in the timeline's row because the chapter panel says the
 * same thing about the same events while a lecture is being chaptered
 * (docs/chapters.md), and two tables would drift into two vocabularies for one
 * `ToolKind`.
 */
export function toolVerb(kind: ToolKind, done: boolean, name?: string | null): string {
  // The one kind two tools share: a search and a fetch are both `web`, and
  // "Fetched" over a list of queries reads as the wrong thing having happened.
  // The provider's own tool name is the only thing that tells them apart, and
  // every row carries it (`name` in `ToolMeta`).
  if (kind === "web" && name && /search/i.test(name)) return done ? "Searched" : "Searching";
  switch (kind) {
    case "read": return done ? "Read" : "Reading";
    case "edit": return done ? "Edited" : "Editing";
    case "write": return done ? "Wrote" : "Writing";
    case "bash": return done ? "Ran" : "Running";
    case "search": return done ? "Searched" : "Searching";
    case "oculus_cli": return done ? "Looked up" : "Looking up";
    case "task": return done ? "Ran subagent" : "Running subagent";
    case "web": return done ? "Fetched" : "Fetching";
    case "plan": return done ? "Updated plan" : "Updating plan";
    default: return done ? "Used" : "Using";
  }
}

export interface RateWindow {
  label: string;
  used_percent: number;
  resets_at: number | null;
}

/** `HarnessEvent` in `app/src-tauri/src/harness/event.rs`, serde-tagged on `type`. */
export type HarnessEvent =
  | { type: "session_started"; provider_session_id: string; model: string | null; cwd: string }
  /** `at` is the playhead's second for a message sent from the lecture dock.
   *  It rides the event rather than only the row Rust wrote, because the
   *  bubble is drawn from the live event first — without it a message showed
   *  its moment only after a reload. Rust skips the field when there is no
   *  moment, so it is optional on the way in too. */
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
  /** `auth` names the provider when the message is that CLI saying it has no
   *  usable credentials — an expired OAuth session, a login never done. It is
   *  the difference between a crash and a state the student can fix, so the
   *  timeline draws a sign-in card rather than a red row for it. Rust writes
   *  the same fact to the row's `meta` (`{"auth":"claude"}`), which is what
   *  `parseErrorMeta` reads after a reload. */
  | { type: "error"; message: string; auth: Provider | null }
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
  /** The recording this conversation is about, for a thread opened in the
   *  lecture player's dock; null for every other thread. Fixed at creation
   *  like the subject beside it — and it is what *sets* that subject, which
   *  Rust reads off the lecture's own row rather than from the payload. */
  lecture_id: string | null;
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

/** What an `error` row's `meta` can carry. Only the one field, and only on
 *  the rows that have it: an error that is not a credentials failure has no
 *  `meta` at all. */
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
    reasoningEfforts: sortReasoning(m.reasoningEfforts),
    defaultReasoningEffort: m.defaultReasoningEffort,
    isDefault: m.isDefault,
  }));
}

/**
 * One row of `opencode models`. The id is that CLI's own spelling,
 * `providerID/id` (`anthropic/claude-sonnet-4-5`) — free-form, and passed back
 * to the CLI untouched, so nothing here parses or prettifies it.
 *
 * opencode calls a model's reasoning levels its **variants**; they are the
 * same thing `claude --effort` and Codex's `reasoningEfforts` name, so the
 * adapter below maps them onto the one field the picker reads. Everything but
 * the id and the name is optional on the way in, the way `getJobModels` is
 * tolerant of a stored row: a bridge that reports no variants for a model
 * costs that model its level row, not the list its rows.
 */
export interface OpencodeModel {
  id: string;
  displayName: string;
  description?: string;
  variants?: string[];
  defaultVariant?: string | null;
  isDefault?: boolean;
  /** `ModelInfo` in `app/src-tauri/src/harness/opencode.rs` always sends
   *  these three; optional here so an older payload still parses. */
  toolCall?: boolean;
  textInput?: boolean;
  textOutput?: boolean;
}

/** `opencode models` in the shape `CLAUDE_MODELS` has, so the picker renders
 *  all three catalogues without a special case. */
export function opencodeAsModels(models: OpencodeModel[]): HarnessModel[] {
  return models.map((m) => {
    const variants = sortReasoning(m.variants ?? []);
    return {
      id: m.id,
      label: m.displayName || m.id,
      description: m.description ?? "",
      reasoningEfforts: variants,
      defaultReasoningEffort: m.defaultVariant ?? variants[0] ?? null,
      isDefault: m.isDefault ?? false,
      toolCall: m.toolCall,
      textInput: m.textInput,
      textOutput: m.textOutput,
    };
  });
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

/**
 * The playhead second a question was asked at, or null for a question asked
 * anywhere but the lecture dock.
 *
 * It is in the user row's `meta` (`{"at": 220}`) rather than in its content,
 * because the moment rides the prompt and never becomes the message — a
 * timeline that read the attachment back as the question would be a timeline
 * of something nobody asked. No cache: `parseToolMeta`'s exists because a
 * single tool row can carry 300KB of output, where this is one number on a
 * memoised row.
 */
export function messageAt(item: HarnessItem): number | null {
  if (!item.meta) return null;
  try {
    const at = (JSON.parse(item.meta) as { at?: unknown }).at;
    return typeof at === "number" ? at : null;
  } catch {
    return null;
  }
}

/**
 * Which agent an error row is about, when the error was that agent saying it
 * has no credentials — and null for every other error.
 *
 * Rust puts it on the row (`{"auth":"claude"}`) as well as on the event, so a
 * reloaded page draws the same card rather than a thread whose one actionable
 * row quietly became a red line again. `isProvider` narrows it, so a row
 * naming an agent this build does not have reads as an ordinary error instead
 * of an unreachable button.
 *
 * No cache, on `messageAt`'s precedent rather than `parseToolMeta`'s: that one
 * exists because a single tool row can carry 300KB of output, where this is
 * one short field on a memoised row.
 */
export function parseErrorMeta(item: HarnessItem): ErrorMeta {
  if (!item.meta) return {};
  try {
    const auth = (JSON.parse(item.meta) as { auth?: unknown }).auth;
    return isProvider(auth) ? { auth } : {};
  } catch {
    return {};
  }
}

// ── Reads ────────────────────────────────────────────────────────────────────

export async function getHarnessThreads(limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(
    `SELECT * FROM harness_threads ORDER BY updated_at DESC, id DESC LIMIT $1`,
    [limit],
  );
}

/** One lecture's threads, newest first — the dock's history list. A separate
 *  query rather than a filter over `getHarnessThreads`, which caps at the
 *  most recent hundred threads across the whole library and would drop a
 *  lecture's older conversations out of its own list. */
export async function getLectureThreads(lectureId: string, limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(
    `SELECT * FROM harness_threads WHERE lecture_id = $1
      ORDER BY updated_at DESC, id DESC LIMIT $2`,
    [lectureId, limit],
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
  /** The lecture a dock thread is about. Only read when the send creates the
   *  thread, and it decides the thread's subject — Rust takes that from the
   *  lecture's row, so `subjectId` is not consulted alongside it. */
  lectureId?: string | null;
  /** The moment, built by the player at send time: the timestamp, the last
   *  minute of transcript, the chapter, and the frame path from
   *  `lectureGrabFrame`. It is appended to the prompt the CLI receives, after
   *  the student's text — it never becomes the message's content, so the
   *  timeline still shows only what was typed. */
  context?: string | null;
  /** The playhead's second when the message was sent. Lands on the user
   *  row's `meta` as `{ at }`, which is what lets the bubble say "at 3:40". */
  at?: number | null;
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

/**
 * Where each CLI is, or why it is not. Read through
 * `app/src/hooks/useBridgeHealth.ts` rather than called directly — every model
 * picker in the app asks this question and there is one answer per session.
 *
 * `recheck` drops Rust's cached lookups first, which can cost a login shell
 * per provider; it belongs to Settings' *Recheck* button and nothing else.
 */
export function harnessHealth(recheck = false): Promise<BridgeHealth[]> {
  return invoke<BridgeHealth[]>("harness_health", { recheck });
}

/** Ask the provider for its plan windows now, rather than waiting for a turn
 *  to report them. The answer comes back as a `rate_limits` event like any
 *  other, so nothing here reads a return value. Codex answers; Claude has no
 *  such request and ignores it. */
export function harnessRefreshRateLimits(provider: Provider): Promise<void> {
  return invoke<void>("harness_refresh_rate_limits", { provider });
}

/**
 * Whether the CLI thinks it is signed in, and as whom.
 *
 * Three fields rather than one boolean because they are three different
 * answers. `signedIn: null` is "not answerable from here" — opencode, whose
 * store is per provider and whose surface is Settings → AI — and is not the
 * same as signed out. `error` is the probe itself failing (no binary, a spawn
 * that would not start), which is not evidence either way.
 *
 * Read through `app/src/hooks/useSignInStatus.ts` rather than called directly:
 * Rust spawns the CLI per ask and caches nothing, so the frontend cache is the
 * only one there is.
 */
export interface SignInStatus {
  provider: Provider;
  signedIn: boolean | null;
  /** Which account the CLI is on — an email where it names one, else the
   *  door it went through ("Claude subscription", "ChatGPT"). */
  account: string | null;
  error: string | null;
}

/** One line of a running sign-in, or the last event of the run. Same shape as
 *  the install stream next door, plus the authorize URL. */
export const SIGNIN_EVENT = "harness-signin";

export interface SignInLine {
  provider: Provider;
  line: string | null;
  /** Emitted once, on the first line that carries one. Rust opens it in the
   *  system browser itself; it rides the event anyway so the dialog can show
   *  it with a Copy, because an `open` that silently failed must not be a
   *  dead end. */
  url: string | null;
  done: boolean;
  ok: boolean | null;
  status: string | null;
}

export function harnessSignInStatus(provider: Provider): Promise<SignInStatus> {
  return invoke<SignInStatus>("harness_sign_in_status", { provider });
}

/** Run the CLI's own login. Output arrives on `SIGNIN_EVENT` until a line
 *  carries `done`; nothing is returned here because the run outlives the call.
 *  Rejects for opencode — see `ProviderInfo.signIn`. */
export function harnessSignInStart(provider: Provider): Promise<void> {
  return invoke("harness_sign_in_start", { provider });
}

/** The authorization code the student pasted back. Only Claude's flow asks
 *  for one: `claude auth login` blocks on stdin until it arrives. */
export function harnessSignInCode(provider: Provider, code: string): Promise<void> {
  return invoke("harness_sign_in_code", { provider, code });
}

export function harnessSignInCancel(provider: Provider): Promise<void> {
  return invoke("harness_sign_in_cancel", { provider });
}

export function harnessCodexModels(): Promise<CodexModel[]> {
  return invoke<CodexModel[]>("harness_codex_models");
}

export function harnessOpencodeModels(): Promise<OpencodeModel[]> {
  return invoke<OpencodeModel[]>("harness_opencode_models");
}

