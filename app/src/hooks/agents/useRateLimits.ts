import { useEffect } from "react";
import { getHarnessRateLimits, harnessRefreshRateLimits, type Provider, type RateWindow } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";

const NONE: RateWindow[] = [];

/** The provider's usage windows, for `UsageMeter`: the stored snapshot at once,
 *  then a refresh (Codex only; Claude's arrive with the next turn). The answer
 *  lands in the store via an event, so the refresh depends on the provider alone. */
export function useRateLimits(provider: Provider): RateWindow[] {
  const windows = useHarnessStore((s) => s.rateLimits[provider]);

  useEffect(() => {
    if (windows) return;
    getHarnessRateLimits(provider).then((w) => {
      if (w.length) useHarnessStore.setState((s) => ({ rateLimits: { ...s.rateLimits, [provider]: w } }));
    });
  }, [provider, windows]);

  useEffect(() => {
    void harnessRefreshRateLimits(provider).catch(() => {});
  }, [provider]);

  return windows ?? NONE;
}
