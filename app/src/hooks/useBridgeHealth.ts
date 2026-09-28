import { useCallback, useEffect } from "react";
import type { BridgeHealth, Provider } from "@/lib/harness";
import { useHarnessHealthStore } from "@/stores/harnessHealthStore";

export type ProviderHealth = "unknown" | "installed" | "missing";

/** Binary discovery and WSL readiness are shared by Settings and every
 * picker. A found binary remains installed even if its readiness probe fails;
 * that actionable failure is rendered separately as unavailableReason. */
export function providerHealth(rows: BridgeHealth[] | null, provider: Provider): ProviderHealth {
  const row = rows?.find((h) => h.provider === provider);
  return row ? (row.path ? "installed" : "missing") : "unknown";
}

export function useBridgeHealth() {
  const health = useHarnessHealthStore((s) => s.health);
  const checking = useHarnessHealthStore((s) => s.checking);
  const error = useHarnessHealthStore((s) => s.error);
  const refresh = useHarnessHealthStore((s) => s.refresh);
  useEffect(() => {
    const check = () => { void refresh(); };
    check();
    window.addEventListener("focus", check);
    return () => window.removeEventListener("focus", check);
  }, [refresh]);
  const recheck = useCallback(() => { void refresh(true); }, [refresh]);
  return { health, checking, error, recheck };
}
