import { invoke } from "@tauri-apps/api/core";
import type { Provider } from "./providers";
import type { ProviderHealth } from "./types";
import type { SettingsPageId } from "@/lib/search/settings";

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
  /** opencode rows only: what Settings → opencode's model table shows. */
  facts?: ModelFacts;
}

/** One model's price, limits and capabilities off opencode's catalogue — a
 *  free local read, never a probe. A field the catalogue omits is null:
 *  unknown, which a price column must not draw as zero. */
export interface ModelFacts {
  /** USD per million tokens. */
  cost: {
    input: number | null;
    output: number | null;
    cacheRead: number | null;
    cacheWrite: number | null;
  } | null;
  context: number | null;
  maxOutput: number | null;
  reasoning: boolean;
  attachment: boolean;
  /** Input modalities besides text: `image`, `audio`, `video`, `pdf`. */
  inputs: string[];
  /** `YYYY-MM-DD`. */
  releaseDate: string | null;
  family: string | null;
}

export interface PickerProvider {
  id: Provider;
  label: string;
  models: HarnessModel[];
  loading?: boolean;
  health?: ProviderHealth;
  emptyNote?: string;
  emptyNotePage?: SettingsPageId;
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

export interface CodexModel {
  id: string;
  displayName: string;
  description: string;
  reasoningEfforts: string[];
  defaultReasoningEffort: string | null;
  isDefault: boolean;
}

/** One row of Claude Code's `/model` catalogue (`list_models` in
 *  `app/src-tauri/src/harness/providers/claude/models.rs`). `value` may be an alias. */
export interface ClaudeModel {
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
export function claudeAsModels(models: ClaudeModel[]): HarnessModel[] {
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

/** One model out of `agy models`. The bridge splits the level suffix off each
 *  slug (`gemini-3.8-flash-high`), so `id` is the base and Rust rebuilds the
 *  full slug at spawn. */
export interface AntigravityModel {
  id: string;
  displayName: string;
  reasoningEfforts: string[];
  defaultReasoningEffort: string | null;
}

export function antigravityAsModels(models: AntigravityModel[]): HarnessModel[] {
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
  facts?: ModelFacts;
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
      facts: m.facts,
    };
  });
}

export function harnessAntigravityModels(): Promise<AntigravityModel[]> {
  return invoke<AntigravityModel[]>("harness_antigravity_models");
}

export function harnessClaudeModels(): Promise<ClaudeModel[]> {
  return invoke<ClaudeModel[]>("harness_claude_models");
}

export function harnessCodexModels(): Promise<CodexModel[]> {
  return invoke<CodexModel[]>("harness_codex_models");
}

export function harnessOpencodeModels(): Promise<OpencodeModel[]> {
  return invoke<OpencodeModel[]>("harness_opencode_models");
}
