/**
 * opencode's provider credentials, frontend side. Everything goes through the
 * app's own `opencode serve` (`app/src-tauri/src/harness/opencode.rs`); nothing
 * writes `auth.json` or keeps a key. The forms are specs declared by opencode,
 * drawn by one generic `OpencodeConnectDialog`.
 */
import { invoke } from "@tauri-apps/api/core";

/** Shows a field only when another answer matches. */
interface AuthWhen {
  key: string;
  op: "eq" | "neq";
  value: string;
}

interface AuthOption {
  label: string;
  value: string;
  hint: string | null;
}

export interface AuthPrompt {
  kind: "text" | "select";
  key: string;
  message: string;
  placeholder: string | null;
  /** Empty unless `kind` is `select`. */
  options: AuthOption[];
  when: AuthWhen | null;
}

export interface AuthMethod {
  /** Position in opencode's array — the only name its OAuth endpoints take. */
  index: number;
  kind: "oauth" | "api";
  label: string;
  /** Extra fields; an `api` method always needs a key on top of these. */
  prompts: AuthPrompt[];
}

export interface OpencodeProvider {
  id: string;
  name: string;
  /** `config`: declared in an `opencode.json`, so no credential to remove. */
  source: string;
  env: string[];
  modelCount: number;
  connected: boolean;
  /** Never empty — a provider that declares nothing takes a plain API key. */
  methods: AuthMethod[];
}

export interface OpencodeProviderList {
  providers: OpencodeProvider[];
  /** A refresh was skipped because a turn was running; the write went through. */
  stale: boolean;
}

export interface Authorization {
  url: string;
  /** `auto`: opencode finishes the flow itself. `code`: the student pastes one. */
  method: "auto" | "code";
  instructions: string;
}

export type Answers = Record<string, string>;

/** `refresh` is needed after a write: in opencode 1.18.2 `PUT /auth` does not
 *  change `GET /provider` for the instance's life. Never call on mount — it
 *  starts `opencode serve`. */
export function opencodeProviders(refresh: boolean): Promise<OpencodeProviderList> {
  return invoke<OpencodeProviderList>("harness_opencode_providers", { refresh });
}

/** The key goes straight to opencode's store; redacted from any error. */
export function opencodeSetKey(
  provider: string,
  method: number,
  key: string,
  answers: Answers,
): Promise<OpencodeProviderList> {
  return invoke<OpencodeProviderList>("harness_opencode_set_key", {
    provider,
    method,
    key,
    answers,
  });
}

export function opencodeDisconnect(provider: string): Promise<OpencodeProviderList> {
  return invoke<OpencodeProviderList>("harness_opencode_disconnect", { provider });
}

export function opencodeOauthStart(
  provider: string,
  method: number,
  answers: Answers,
): Promise<Authorization> {
  return invoke<Authorization>("harness_opencode_oauth_start", { provider, method, answers });
}

export function opencodeOauthFinish(
  provider: string,
  method: number,
  code: string | null,
): Promise<OpencodeProviderList> {
  return invoke<OpencodeProviderList>("harness_opencode_oauth_finish", {
    provider,
    method,
    code,
  });
}

/** Which fields are on screen. Drawing only — Rust re-applies the rule so an
 *  abandoned answer is never sent. */
export function visiblePrompts(method: AuthMethod, answers: Answers): AuthPrompt[] {
  return method.prompts.filter((p) => {
    if (!p.when) return true;
    const actual = answers[p.when.key] ?? "";
    if (p.when.op === "eq") return actual === p.when.value;
    if (p.when.op === "neq") return actual !== p.when.value;
    return true;
  });
}

export function formComplete(method: AuthMethod, answers: Answers, key: string): boolean {
  if (method.kind === "api" && !key.trim()) return false;
  return visiblePrompts(method, answers).every((p) => (answers[p.key] ?? "").trim().length > 0);
}

/** Preselect each select's first option; opencode declares no defaults. */
export function initialAnswers(method: AuthMethod): Answers {
  const out: Answers = {};
  for (const p of method.prompts) {
    if (p.kind === "select" && p.options[0]) out[p.key] = p.options[0].value;
  }
  return out;
}

/** The session's last read, so reopening Settings starts no CLI. */
let cached: OpencodeProviderList | null = null;

export function cachedProviders(): OpencodeProviderList | null {
  return cached;
}

export function rememberProviders(list: OpencodeProviderList): OpencodeProviderList {
  cached = list;
  return list;
}

export function forgetProviders(): void {
  cached = null;
}
