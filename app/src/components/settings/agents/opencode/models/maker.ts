import type { HarnessModel } from "@/lib/harness";
import { humanizeSlug } from "@/lib/format/format";

/**
 * Who made a model, as against the provider serving it: `anthropic` for both
 * `anthropic/claude-opus-4-5` and `openrouter/~anthropic/claude-opus-latest`.
 * Keys are OpenRouter's author slugs, since it lists the most makers.
 */

/** Spellings of one maker across providers, folded into OpenRouter's slug. */
const ALIASES: Record<string, string> = {
  zai: "z-ai",
  "zai-coding-plan": "z-ai",
  xai: "x-ai",
  mistral: "mistralai",
  meta: "meta-llama",
  "bytedance-seed": "bytedance",
  "moonshotai-cn": "moonshotai",
  "google-vertex": "google",
  "google-vertex-anthropic": "anthropic",
  "amazon-bedrock": "amazon",
};

/** For a bare model name (`kimi-k3` from OpenCode Go), the maker its family or
 *  name points to. */
const FAMILIES: ReadonlyArray<[RegExp, string]> = [
  [/^claude/, "anthropic"],
  [/^(gpt|chatgpt|codex|o\d|whisper)/, "openai"],
  [/^(gemini|gemma)/, "google"],
  [/^(qwen|qwq)/, "qwen"],
  [/^glm/, "z-ai"],
  [/^kimi/, "moonshotai"],
  [/^deepseek/, "deepseek"],
  [/^grok/, "x-ai"],
  [/^(llama|muse)/, "meta-llama"],
  [/^(mistral|ministral|codestral|devstral|magistral|pixtral)/, "mistralai"],
  [/^mimo/, "xiaomi"],
  [/^minimax/, "minimax"],
  [/^nemotron/, "nvidia"],
  [/^command/, "cohere"],
  [/^(seed|doubao)/, "bytedance"],
  [/^(ling|ring)-/, "inclusionai"],
  [/^longcat/, "meituan"],
  [/^hy(\d|$)/, "tencent"],
  [/^step-/, "stepfun"],
  [/^mai(-|$)/, "microsoft"],
];

/** Providers that only serve their own models. */
const FIRST_PARTY = new Set([
  "anthropic",
  "openai",
  "google",
  "deepseek",
  "moonshotai",
  "z-ai",
  "x-ai",
  "mistralai",
  "minimax",
  "xiaomi",
  "cohere",
]);

const NAMES: Record<string, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  google: "Google",
  qwen: "Qwen",
  "z-ai": "Z.ai",
  moonshotai: "Moonshot AI",
  deepseek: "DeepSeek",
  "x-ai": "xAI",
  "meta-llama": "Meta",
  mistralai: "Mistral",
  xiaomi: "Xiaomi",
  minimax: "MiniMax",
  nvidia: "NVIDIA",
  cohere: "Cohere",
  bytedance: "ByteDance",
  inclusionai: "inclusionAI",
  meituan: "Meituan",
  openrouter: "OpenRouter",
  "ibm-granite": "IBM Granite",
  ai21: "AI21",
  stepfun: "StepFun",
  thinkingmachines: "Thinking Machines",
};

const canon = (slug: string) => {
  const s = slug.replace(/^~/, "").toLowerCase();
  return ALIASES[s] ?? s;
};

/** The maker's slug, or null when neither the id nor the family names one. */
export function makerOf(model: HarnessModel): string | null {
  const slash = model.id.indexOf("/");
  const provider = canon(model.id.slice(0, slash));
  const name = model.id.slice(slash + 1);
  const inner = name.indexOf("/");
  if (inner > 0) return canon(name.slice(0, inner));
  if (FIRST_PARTY.has(provider)) return provider;
  for (const hint of [model.facts?.family, name]) {
    if (!hint) continue;
    for (const [re, maker] of FAMILIES) if (re.test(hint.toLowerCase())) return maker;
  }
  return null;
}

export function makerName(slug: string): string {
  return NAMES[slug] ?? humanizeSlug(slug);
}
