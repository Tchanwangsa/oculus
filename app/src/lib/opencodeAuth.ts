/**
 * opencode's provider credentials, frontend side.
 *
 * Claude Code and Codex are signed in with their own CLIs and carry the
 * student's subscription; opencode reaches every provider at once and reaches
 * exactly the ones `opencode auth` holds a credential for. Until this existed
 * the only way to put one there was a terminal, so the model picker showed two
 * providers out of two hundred and eighteen with nothing on screen saying why.
 *
 * Everything here goes through the app's own `opencode serve`
 * (`app/src-tauri/src/harness/opencode.rs`), which is the supported door to
 * that store — and, for the browser flows, the process that owns the loopback
 * listener the redirect comes back to. Nothing writes `auth.json` directly and
 * nothing here mirrors a credential into the app: a key is a function argument
 * on its way to one request and is not in this module a moment longer.
 *
 * The forms are **declared by opencode**, not written here. A method is a spec
 * — a kind, a label, and a list of prompts with optional conditions — so one
 * generic dialog (`app/src/components/settings/OpencodeConnectDialog.tsx`)
 * covers `openai`'s three ways in, `github-copilot`'s enterprise branch and
 * the two hundred providers that just take a key, including ones opencode adds
 * after this was written.
 */
import { invoke } from "@tauri-apps/api/core";

/** Shows a field only when another answer matches. */
export interface AuthWhen {
  key: string;
  op: "eq" | "neq";
  value: string;
}

export interface AuthOption {
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
  /** Position in opencode's own array for this provider, and the only name
   *  the OAuth endpoints have for a method. Passed back verbatim. */
  index: number;
  kind: "oauth" | "api";
  label: string;
  /** The *extra* fields. An `api` method always needs a key on top of these;
   *  `openai`'s "Manually enter API Key" declares no prompts at all. */
  prompts: AuthPrompt[];
}

export interface OpencodeProvider {
  id: string;
  name: string;
  /** `custom` is opencode's built-in catalogue; `config` means the provider is
   *  declared in an `opencode.json` rather than signed in to, so there is no
   *  credential to remove. */
  source: string;
  env: string[];
  modelCount: number;
  connected: boolean;
  /** Never empty — a provider that declares nothing takes a plain API key. */
  methods: AuthMethod[];
}

export interface OpencodeProviderList {
  providers: OpencodeProvider[];
  /** A refresh was skipped because an opencode turn was running. The write
   *  itself went through; the server has simply not re-read its store yet. */
  stale: boolean;
}

export interface Authorization {
  url: string;
  /** `auto` — opencode finishes the flow itself, on a loopback listener or a
   *  device poll, and the app watches for the credential to appear. `code` —
   *  the student pastes something back. */
  method: "auto" | "code";
  instructions: string;
}

export type Answers = Record<string, string>;

/**
 * The list, and optionally a re-read first.
 *
 * `refresh` is not free and is not cosmetic: measured against opencode 1.18.2,
 * a credential written through `PUT /auth` does not change what `GET /provider`
 * reports for the life of the instance, so every read after a write asks for
 * one. It is also what this section must **not** do on mount — the call starts
 * `opencode serve` if it is down, and a settings page being opened is not a
 * reason to spawn a CLI.
 */
export function opencodeProviders(refresh: boolean): Promise<OpencodeProviderList> {
  return invoke<OpencodeProviderList>("harness_opencode_providers", { refresh });
}

/** The key goes straight to opencode's store. It is never returned, never
 *  stored by the app, and redacted out of any error on the way back. */
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

/**
 * Which of a method's fields are on screen, given what has been answered.
 *
 * The same rule runs again in Rust before anything is sent — an abandoned
 * answer must not travel — so this copy is the *drawing* half only: it decides
 * what the dialog shows and what counts as filled in. An unanswered dependency
 * reads as the empty string, which is why `github-copilot`'s enterprise URL is
 * absent until the select says so.
 */
export function visiblePrompts(method: AuthMethod, answers: Answers): AuthPrompt[] {
  return method.prompts.filter((p) => {
    if (!p.when) return true;
    const actual = answers[p.when.key] ?? "";
    if (p.when.op === "eq") return actual === p.when.value;
    if (p.when.op === "neq") return actual !== p.when.value;
    return true;
  });
}

/** Whether the form can be submitted: every visible field answered, plus the
 *  key an `api` method always needs on top of its prompts. */
export function formComplete(method: AuthMethod, answers: Answers, key: string): boolean {
  if (method.kind === "api" && !key.trim()) return false;
  return visiblePrompts(method, answers).every((p) => (answers[p.key] ?? "").trim().length > 0);
}

/** A select's first option, so a branch is never drawn with nothing chosen —
 *  opencode's own prompts assume the obvious default (`github.com`) rather
 *  than declaring one. */
export function initialAnswers(method: AuthMethod): Answers {
  const out: Answers = {};
  for (const p of method.prompts) {
    if (p.kind === "select" && p.options[0]) out[p.key] = p.options[0].value;
  }
  return out;
}

/**
 * The list this session has already read, so reopening Settings draws it
 * without another invoke.
 *
 * `useBridgeHealth`'s shape and its reason: the first read is the one that may
 * start a CLI, and it happens on a click. After that the server is up and the
 * answer is worth keeping for as long as the window lives.
 */
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
