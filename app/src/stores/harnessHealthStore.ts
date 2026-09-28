import { create } from "zustand";
import { harnessHealth, type BridgeHealth } from "@/lib/harness";

const RECHECK_AFTER_MS = 30_000;

interface HarnessHealthState {
  health: BridgeHealth[] | null;
  checking: boolean;
  error: string | null;
  checkedAt: number;
  refresh: (force?: boolean) => Promise<void>;
}

/** Chat, lecture chat and Settings share the same answer. Rechecking after an
 * install or login must enable every composer without replacing its draft or
 * selection. Focus checks are throttled and simultaneous checks are coalesced. */
export const useHarnessHealthStore = create<HarnessHealthState>((set, get) => ({
  health: null,
  checking: false,
  error: null,
  checkedAt: 0,
  refresh: async (force = false) => {
    const state = get();
    if (state.checking || (!force && Date.now() - state.checkedAt < RECHECK_AFTER_MS)) return;
    set({ checking: true });
    try {
      set({ health: await harnessHealth(force), error: null, checkedAt: Date.now() });
    } catch (error) {
      set({ error: `Could not check CLI agents: ${String(error)}`, checkedAt: Date.now() });
    } finally {
      set({ checking: false });
    }
  },
}));

/** A missing or failed WSL probe cannot become an optimistic native launch.
 * The last selection stays visible so existing threads can recover on recheck. */
export function claudeWslUnavailableReason(
  health: BridgeHealth[] | null,
  error: string | null,
): string | undefined {
  if (error) return `${error} Recheck in Settings → AI.`;
  if (health === null) return "Checking Claude Code in WSL2…";
  const claude = health.find((entry) => entry.provider === "claude");
  if (claude?.error) return claude.error;
  if (!claude?.path) return "Claude Code in WSL2 is not ready. Recheck in Settings → AI for setup details.";
  return undefined;
}
