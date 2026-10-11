import {
  antigravityAsModels,
  claudeAsModels,
  codexAsModels,
  defaultSelection,
  harnessAntigravityModels,
  harnessClaudeModels,
  harnessCodexModels,
  harnessOpencodeModels,
  opencodeAsModels,
  type HarnessModel,
} from "./models";
import type { Provider } from "./providers";
import { filterOffered, loadCatalogue } from "@/lib/harness/opencodeCatalogue";
import type { SettingsPageId } from "@/lib/search/settings";

export interface ProviderInfo {
  id: Provider;
  label: string;
  /** A compiled-in catalogue; null when the CLI reports its own. */
  staticModels: HarnessModel[] | null;
  /** Present exactly when `staticModels` is null; already in picker shape. */
  fetchModels?: () => Promise<HarnessModel[]>;
  /** How the CLI's sign-in ends: `"code"` blocks on a pasted code (Claude),
   *  `"callback"` finishes via a loopback server (Codex). `null`: nothing to
   *  drive here — opencode signs in per provider in Settings → opencode. */
  signIn: "code" | "callback" | null;
  /** What the picker says when an installed CLI's list is empty because of a
   *  step the student can take. */
  emptyNote?: string;
  /** The Settings page the empty-list note's button opens. */
  emptyNotePage?: SettingsPageId;
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
    emptyNote: "Sign in to a provider in Settings → opencode to get models here.",
    emptyNotePage: "opencode",
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
    emptyNotePage: "agents",
    rewind: false,
  },
];

export function providerInfo(provider: Provider): ProviderInfo | undefined {
  return PROVIDERS.find((p) => p.id === provider);
}

export function providerLabel(provider: Provider): string {
  return providerInfo(provider)?.label ?? provider;
}

export function signInFlow(provider: Provider): "code" | "callback" | null {
  return providerInfo(provider)?.signIn ?? null;
}

/** Empty for a fetched catalogue; the picker fills it when the list lands. */
export function defaultSelectionFor(provider: Provider): {
  model: string | null;
  reasoning: string | null;
} {
  const models = providerInfo(provider)?.staticModels;
  return models ? defaultSelection(models) : { model: null, reasoning: null };
}
