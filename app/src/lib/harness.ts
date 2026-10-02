/**
 * The CLI-agent harness, frontend side: types mirroring the Rust `harness`
 * module, reads over its tables, and its commands. Rust writes every row;
 * `stores/harnessStore.ts` folds the live events. See docs/harness.md.
 */
import { invoke } from "@tauri-apps/api/core";
import { getDb, getSetting } from "@/lib/db";
import { filterOffered, loadCatalogue } from "@/lib/opencodeCatalogue";

export type Provider = "claude" | "codex" | "opencode" | "antigravity";

/** Reasoning levels, **weakest first**: the key order is the canonical one
 *  `sortReasoning` uses. Levels are per model, not per provider. */
const REASONING_LABELS: Record<string, string> = {
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

const REASONING_ORDER = Object.keys(REASONING_LABELS);

/** Weakest to strongest; no provider sends them sorted (opencode's arrive
 *  alphabetically). Unknown levels keep their order at the end. */
function sortReasoning(levels: string[]): string[] {
  const rank = (l: string) => {
    const i = REASONING_ORDER.indexOf(l);
    return i === -1 ? REASONING_ORDER.length : i;
  };
  return [...levels].sort((a, b) => rank(a) - rank(b));
}

export interface HarnessModel {
  /** Passed verbatim to the CLI. */
  id: string;
  /** The model's own name, never a bare alias, without a brand prefix. */
  label: string;
  description: string;
  reasoningEfforts: string[];
  defaultReasoningEffort: string | null;
  /** Where the composer starts; every turn still names its model. */
  isDefault?: boolean;
  /** opencode rows only; absent reads as capable. See `unusableReason`. */
  toolCall?: boolean;
  textInput?: boolean;
  textOutput?: boolean;
}

/** Whether a provider's CLI is available to model pickers. */
export type ProviderHealth = "unknown" | "installed" | "missing";

export interface PickerProvider {
  id: Provider;
  label: string;
  models: HarnessModel[];
  loading?: boolean;
  health?: ProviderHealth;
  emptyNote?: string;
}

/** The model and level a fresh composer opens on. */
export function defaultSelection(models: HarnessModel[]): {
  model: string | null;
  reasoning: string | null;
} {
  const m = models.find((x) => x.isDefault) ?? models[0];
  if (!m) return { model: null, reasoning: null };
  return { model: m.id, reasoning: m.defaultReasoningEffort ?? m.reasoningEfforts[0] ?? null };
}

export interface ProviderInfo {
  id: Provider;
  label: string;
  /** A compiled-in catalogue; null when the CLI reports its own. */
  staticModels: HarnessModel[] | null;
  /** Present exactly when `staticModels` is null; already in picker shape. */
  fetchModels?: () => Promise<HarnessModel[]>;
  /** How the CLI's sign-in ends: `"code"` blocks on a pasted code (Claude),
   *  `"callback"` finishes via a loopback server (Codex). `null`: nothing to
   *  drive here — opencode signs in per provider in Settings → AI. */
  signIn: "code" | "callback" | null;
  /** What the picker says when an installed CLI's list is empty because of a
   *  step the student can take. */
  emptyNote?: string;
  /** Whether the CLI can drop a question from its own context — gates Rewind,
   *  Edit and Retry. `agy` 1.2.9 cannot in print mode. */
  rewind: boolean;
}

/** Every provider, in picker order — the one place a provider is declared;
 *  call sites read these fields rather than testing ids. */
export const PROVIDERS: ProviderInfo[] = [
  {
    id: "claude",
    label: "Claude Code",
    staticModels: null,
    fetchModels: () => harnessClaudeModels().then(claudeAsModels),
    signIn: "code",
    rewind: true,
  },
  {
    id: "codex",
    label: "Codex",
    staticModels: null,
    fetchModels: () => harnessCodexModels().then(codexAsModels),
    signIn: "callback",
    rewind: true,
  },
  {
    id: "opencode",
    label: "opencode",
    staticModels: null,
    // opencode's catalogue lists models that do not work; filtered here so
    // callers stay provider-blind (see `filterOffered`).
    fetchModels: async () => {
      const models = opencodeAsModels(await harnessOpencodeModels());
      return filterOffered(models, await loadCatalogue());
    },
    signIn: null,
    emptyNote: "Sign in to a provider in Settings → AI to get models here.",
    rewind: true,
  },
  {
    id: "antigravity",
    label: "Antigravity",
    staticModels: null,
    // `agy models` lists the account's own entitlements, so no filter.
    fetchModels: () => harnessAntigravityModels().then(antigravityAsModels),
    // `agy` has no login subcommand; signing in is its interactive CLI.
    signIn: null,
    emptyNote:
      "Run agy in a terminal and finish its Google sign-in, then its models appear here.",
    rewind: false,
  },
];

export function providerInfo(provider: Provider): ProviderInfo | undefined {
  return PROVIDERS.find((p) => p.id === provider);
}

export function providerLabel(provider: Provider): string {
  return providerInfo(provider)?.label ?? provider;
}

/**
 * The route for a Chat tab showing one conversation, or with no id the empty
 * composer. The thread lives only in the route: every tab has its own router
 * (`TabPane`), so two tabs hold two conversations and back walks between them.
 * `n` is the name, because `tabInfo` titles a tab from its path alone.
 */
export function chatHref(threadId?: number | null, title?: string | null): string {
  if (threadId == null) return "/chat";
  const name = title?.trim();
  return name ? `/chat?t=${threadId}&n=${encodeURIComponent(name)}` : `/chat?t=${threadId}`;
}

/** The thread id a Chat route names, or null for the empty composer. */
export function chatThreadId(search: string): number | null {
  const raw = new URLSearchParams(search).get("t");
  const id = raw == null ? NaN : Number(raw);
  return Number.isInteger(id) && id > 0 ? id : null;
}

export function signInFlow(provider: Provider): "code" | "callback" | null {
  return providerInfo(provider)?.signIn ?? null;
}

/** Narrow a stored string to a provider this build has. */
export function isProvider(value: unknown): value is Provider {
  return typeof value === "string" && PROVIDERS.some((p) => p.id === value);
}

/** Empty for a fetched catalogue; the picker fills it when the list lands. */
export function defaultSelectionFor(provider: Provider): {
  model: string | null;
  reasoning: string | null;
} {
  const models = providerInfo(provider)?.staticModels;
  return models ? defaultSelection(models) : { model: null, reasoning: null };
}

const TOOL_KINDS = ["read", "edit", "write", "bash", "search", "oculus_cli", "task", "web", "plan", "other"] as const;
export type ToolKind = (typeof TOOL_KINDS)[number];

/** "Ran", "Read", "Edited" — past tense once done. Shared with the chapter
 *  panel so both speak one vocabulary. */
export function toolVerb(kind: ToolKind, done: boolean, name?: string | null): string {
  // Search and fetch are both `web`; only the tool name tells them apart.
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

interface CodexModel {
  id: string;
  displayName: string;
  description: string;
  reasoningEfforts: string[];
  defaultReasoningEffort: string | null;
  isDefault: boolean;
}

/** One row of Claude Code's `/model` catalogue (`list_models` in
 *  `app/src-tauri/src/harness/claude.rs`). `value` may be an alias. */
interface ClaudeModel {
  value: string;
  resolvedModel: string;
  displayName: string;
  description: string;
  /** Empty for a model that takes no `--effort` (Haiku). */
  supportedEffortLevels?: string[];
}

/** "Opus 5.5 with 1M context · Best for…" → "Opus 5.5". */
const CLAUDE_NAME = /^([A-Za-z]+)\s+(\d+(?:\.\d+)*)/;

/** The id is a real model name, never a moving alias — a stored `sonnet`
 *  would change model under a saved thread. A full-name `value` is kept for
 *  its `[1m]`, which `resolvedModel` drops. Defaults to medium like `/model`. */
function claudeAsModels(models: ClaudeModel[]): HarnessModel[] {
  const out: HarnessModel[] = [];
  const seen = new Set<string>();
  for (const m of models) {
    const id = m.value?.startsWith("claude-") ? m.value : m.resolvedModel;
    if (!id || seen.has(id)) continue;
    seen.add(id);
    const description = m.description ?? "";
    const name = CLAUDE_NAME.exec(description);
    const sep = description.indexOf(" · ");
    const efforts = sortReasoning(m.supportedEffortLevels ?? []);
    out.push({
      id,
      label: name ? `${name[1]} ${name[2]}` : m.displayName || id,
      description: sep === -1 ? description : description.slice(sep + 3),
      reasoningEfforts: efforts,
      defaultReasoningEffort: efforts.includes("medium") ? "medium" : (efforts[0] ?? null),
      isDefault: m.value === "default",
    });
  }
  return out;
}

function codexAsModels(models: CodexModel[]): HarnessModel[] {
  return models.map((m) => ({
    id: m.id,
    label: m.displayName,
    description: m.description,
    reasoningEfforts: sortReasoning(m.reasoningEfforts),
    defaultReasoningEffort: m.defaultReasoningEffort,
    isDefault: m.isDefault,
  }));
}

/** One model out of `agy models`. The bridge splits the level suffix off each
 *  slug (`gemini-3.8-flash-high`), so `id` is the base and Rust rebuilds the
 *  full slug at spawn. */
interface AntigravityModel {
  id: string;
  displayName: string;
  reasoningEfforts: string[];
  defaultReasoningEffort: string | null;
}

function antigravityAsModels(models: AntigravityModel[]): HarnessModel[] {
  return models.map((m) => ({
    id: m.id,
    label: m.displayName || m.id,
    description: "",
    reasoningEfforts: sortReasoning(m.reasoningEfforts),
    defaultReasoningEffort: m.defaultReasoningEffort,
  }));
}

/** One row of `opencode models`. `id` is `providerID/id`, passed back to the
 *  CLI untouched; `variants` are its reasoning levels. */
export interface OpencodeModel {
  id: string;
  displayName: string;
  description?: string;
  variants?: string[];
  defaultVariant?: string | null;
  isDefault?: boolean;
  toolCall?: boolean;
  textInput?: boolean;
  textOutput?: boolean;
}

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

/** Row objects are replaced on change, so parsed metadata invalidates itself.
 *  Non-object JSON is rejected before any row reads its fields. */
const ITEM_META = new WeakMap<HarnessItem, Record<string, unknown>>();

export function parseItemMeta(item: HarnessItem): Record<string, unknown> {
  const hit = ITEM_META.get(item);
  if (hit) return hit;
  let meta: Record<string, unknown> = {};
  try {
    const parsed: unknown = JSON.parse(item.meta ?? "{}");
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      meta = parsed as Record<string, unknown>;
    }
  } catch {
    // Invalid metadata leaves the row readable, without actions to approve.
  }
  ITEM_META.set(item, meta);
  return meta;
}

const TOOL_META = new WeakMap<HarnessItem, ToolMeta>();

export function parseToolMeta(item: HarnessItem): ToolMeta {
  const hit = TOOL_META.get(item);
  if (hit) return hit;
  const meta: Record<string, unknown> = { ...parseItemMeta(item) };
  // An unknown kind has no icon; invalid outputs cannot be rendered as text.
  if (!TOOL_KINDS.includes(meta.kind as ToolKind)) delete meta.kind;
  if (typeof meta.name !== "string") delete meta.name;
  if (meta.ok != null && typeof meta.ok !== "boolean") delete meta.ok;
  if (meta.output != null && typeof meta.output !== "string") delete meta.output;
  TOOL_META.set(item, meta as ToolMeta);
  return meta as ToolMeta;
}

/** The playhead second a lecture-dock question was asked at, from the user
 *  row's `meta` (`{"at": 220}`); null elsewhere. */
export function messageAt(item: HarnessItem): number | null {
  const { at } = parseItemMeta(item);
  return typeof at === "number" ? at : null;
}

/** The agent a credentials-failure error row is about (`{"auth":"claude"}`);
 *  an unknown provider reads as an ordinary error. */
export function parseErrorMeta(item: HarnessItem): ErrorMeta {
  const { auth } = parseItemMeta(item);
  return isProvider(auth) ? { auth } : {};
}

/** A `permission` row's `meta`. A row that does not parse draws as a refusal
 *  with nothing to approve, never a malformed rule. */
export interface PermissionMeta {
  tool?: string;
  action?: string;
  target?: string | null;
  rule?: string | null;
}

export function parsePermissionMeta(item: HarnessItem): PermissionMeta {
  const m = parseItemMeta(item);
  const str = (v: unknown) => (typeof v === "string" && v ? v : null);
  return {
    tool: str(m.tool) ?? undefined,
    action: str(m.action) ?? undefined,
    target: str(m.target),
    rule: str(m.rule),
  };
}

/** An `agy` rule taken apart; null outside the shapes `is_valid_rule` in
 *  `app/src-tauri/src/harness/antigravity_rules.rs` accepts. */
export function splitRule(
  rule: string,
): { action: "command" | "read_file" | "write_file" | "read_url"; value: string } | null {
  const m = /^(command|read_file|write_file|read_url)\((.+)\)$/.exec(rule);
  if (!m) return null;
  return { action: m[1] as "command" | "read_file" | "write_file" | "read_url", value: m[2] };
}

// ── Reads ────────────────────────────────────────────────────────────────────

export async function getHarnessThreads(limit = 100): Promise<HarnessThread[]> {
  const db = await getDb();
  return db.select<HarnessThread[]>(
    `SELECT * FROM harness_threads ORDER BY updated_at DESC, id DESC LIMIT $1`,
    [limit],
  );
}

/** One lecture's threads — its own query, since `getHarnessThreads` caps
 *  library-wide and would drop a lecture's older ones. */
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
  /** Only read when the send creates the thread. */
  subjectId?: number | null;
  /** Only read on creation; overrides `subjectId`. */
  lectureId?: string | null;
  /** The lecture moment, appended to the prompt but never to the message. */
  context?: string | null;
  /** Playhead second; stored on the user row's `meta` as `{ at }`. */
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

/** Rewind to a question and send new text in its place. See docs/harness.md
 *  "Going back" for when the agent's own context cannot follow. */
export function harnessEditResend(
  threadId: number,
  itemId: number,
  text: string,
  options: SendOptions,
): Promise<void> {
  return invoke("harness_edit_resend", { threadId, itemId, text, options });
}

/** `harnessEditResend` without the send: resolves to the question's text. */
export function harnessRewind(threadId: number, itemId: number): Promise<string> {
  return invoke<string>("harness_rewind", { threadId, itemId });
}

export function harnessQueued(threadId: number): Promise<QueuedMessage[]> {
  return invoke<QueuedMessage[]>("harness_queued", { threadId });
}

export function harnessUnqueue(threadId: number, queueId: string): Promise<void> {
  return invoke("harness_unqueue", { threadId, queueId });
}

export function harnessEditQueued(threadId: number, queueId: string, text: string): Promise<void> {
  return invoke("harness_edit_queued", { threadId, queueId, text });
}

/** Stop the turn and clear the queue; resolves to the dropped messages. */
export function harnessInterrupt(threadId: number): Promise<string[]> {
  return invoke<string[]>("harness_interrupt", { threadId });
}

export function harnessDeleteThread(threadId: number): Promise<void> {
  return invoke("harness_delete_thread", { threadId });
}

/** Read through `useBridgeHealth`. `recheck` drops Rust's cached lookups
 *  (a login shell per provider) — Settings' Recheck button only. */
export function harnessHealth(recheck = false): Promise<BridgeHealth[]> {
  return invoke<BridgeHealth[]>("harness_health", { recheck });
}

/** The answer arrives as a `rate_limits` event. Claude ignores it. */
export function harnessRefreshRateLimits(provider: Provider): Promise<void> {
  return invoke<void>("harness_refresh_rate_limits", { provider });
}

/** Read through `useSignInStatus` (Rust caches nothing). `signedIn: null` is
 *  "not answerable here" (opencode); `error` is the check itself failing. */
export interface SignInStatus {
  provider: Provider;
  signedIn: boolean | null;
  /** An email, else the route ("Claude subscription", "ChatGPT"). */
  account: string | null;
  error: string | null;
}

export const SIGNIN_EVENT = "harness-signin";

export interface SignInLine {
  provider: Provider;
  line: string | null;
  /** Rust opens it too; shown with Copy in case that `open` failed. */
  url: string | null;
  done: boolean;
  ok: boolean | null;
  status: string | null;
}

export function harnessSignInStatus(provider: Provider): Promise<SignInStatus> {
  return invoke<SignInStatus>("harness_sign_in_status", { provider });
}

/** Output streams on `SIGNIN_EVENT` until `done`. Rejects for opencode. */
export function harnessSignInStart(provider: Provider): Promise<void> {
  return invoke("harness_sign_in_start", { provider });
}

export function harnessSignInCode(provider: Provider, code: string): Promise<void> {
  return invoke("harness_sign_in_code", { provider, code });
}

export function harnessSignInCancel(provider: Provider): Promise<void> {
  return invoke("harness_sign_in_cancel", { provider });
}

function harnessAntigravityModels(): Promise<AntigravityModel[]> {
  return invoke<AntigravityModel[]>("harness_antigravity_models");
}

/** Store the rule and drop the thread's `agy` process (it never re-reads its
 *  rules); the caller's follow-up resumes under them. Resolves to all rules. */
export function harnessAntigravityAllow(threadId: number, rule: string): Promise<string[]> {
  return invoke<string[]>("harness_antigravity_allow", { threadId, rule });
}

export function harnessAntigravityRules(): Promise<string[]> {
  return invoke<string[]>("harness_antigravity_rules");
}

export function harnessAntigravityRevoke(rule: string): Promise<string[]> {
  return invoke<string[]>("harness_antigravity_revoke", { rule });
}

function harnessClaudeModels(): Promise<ClaudeModel[]> {
  return invoke<ClaudeModel[]>("harness_claude_models");
}

function harnessCodexModels(): Promise<CodexModel[]> {
  return invoke<CodexModel[]>("harness_codex_models");
}

export function harnessOpencodeModels(): Promise<OpencodeModel[]> {
  return invoke<OpencodeModel[]>("harness_opencode_models");
}

