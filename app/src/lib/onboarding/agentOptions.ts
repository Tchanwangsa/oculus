import type { Provider } from "@/lib/harness/providers";

/** One agent as the onboarding Agent step offers it: what it costs and where
 *  to get it. */
export interface AgentOption {
  provider: Provider;
  /** The card's detail line. */
  cost: string;
  /** A second, quieter line: the next plan up, or the free tier's limits. */
  note: string;
  link: { label: string; url: string };
}

// Prices from the vendors' own pages on 2026-10-12, in USD (none lists AUD);
// numbers a vendor doesn't publish are left out (docs/onboarding.md).
const OPTIONS: Record<Provider, AgentOption> = {
  claude: {
    provider: "claude",
    cost: "Needs a Claude Pro (US$20/month) or Max plan.",
    note: "The free Claude plan doesn't include it. Max starts at US$100/month.",
    link: { label: "Plans", url: "https://claude.com/pricing" },
  },
  codex: {
    provider: "codex",
    cost: "Works on a free ChatGPT account, with tight limits. Plus is US$20/month.",
    note: "ChatGPT Go, US$8/month, includes it too.",
    link: { label: "Plans", url: "https://chatgpt.com/pricing" },
  },
  antigravity: {
    provider: "antigravity",
    cost: "Free with a Google account, with weekly limits.",
    note: "A Google AI Pro plan raises the limits.",
    link: { label: "Plans", url: "https://antigravity.google" },
  },
  opencode: {
    provider: "opencode",
    cost: "Bring an OpenRouter key and pay as you go.",
    note: "Some models are free: 50 requests a day, 1,000 after a one-off US$10 top-up.",
    link: { label: "Get a key", url: "https://openrouter.ai/keys" },
  },
};

/** Fixed and neutral: no agent is pushed as the pick. */
export const AGENT_ORDER: readonly Provider[] = ["claude", "codex", "antigravity", "opencode"];

export function agentOption(provider: Provider): AgentOption {
  return OPTIONS[provider];
}

/** `AGENT_ORDER` with the ready agents moved to the front, each half keeping
 *  its order. */
export function orderAgents(isReady: (provider: Provider) => boolean): Provider[] {
  return [...AGENT_ORDER.filter(isReady), ...AGENT_ORDER.filter((p) => !isReady(p))];
}
