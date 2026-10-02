import { useCallback, useEffect, useState } from "react";

import { harnessHealth, type BridgeHealth, type Provider, type ProviderHealth } from "@/lib/harness";

/** Whether each provider's CLI is on this machine, shared by every picker.
 *  Cached here (one in-flight invoke) and in Rust (`harness/discover.rs`);
 *  only Settings' Recheck clears both. */
export type { ProviderHealth } from "@/lib/harness";

let cached: BridgeHealth[] | null = null;
let inFlight: Promise<BridgeHealth[]> | null = null;
const listeners = new Set<(rows: BridgeHealth[]) => void>();

function read(recheck: boolean): Promise<BridgeHealth[]> {
  if (!recheck) {
    if (cached) return Promise.resolve(cached);
    if (inFlight) return inFlight;
  }
  // A failed check reads as `unknown`, not as missing.
  const p = harnessHealth(recheck)
    .catch(() => [] as BridgeHealth[])
    .then((rows) => {
      cached = rows;
      inFlight = null;
      for (const l of listeners) l(rows);
      return rows;
    });
  inFlight = p;
  return p;
}

export function providerHealth(rows: BridgeHealth[] | null, provider: Provider): ProviderHealth {
  if (!rows) return "unknown";
  const row = rows.find((h) => h.provider === provider);
  if (!row) return "unknown";
  // Found but failing `--version` still counts as installed; Settings shows why.
  return row.path ? "installed" : "missing";
}

export function useBridgeHealth(): {
  health: BridgeHealth[] | null;
  /** Settings' Recheck button only: Rust drops its cached lookups. */
  recheck: () => void;
  checking: boolean;
} {
  const [health, setHealth] = useState<BridgeHealth[] | null>(cached);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    listeners.add(setHealth);
    let live = true;
    void read(false).then((rows) => {
      if (live) setHealth(rows);
    });
    return () => {
      live = false;
      listeners.delete(setHealth);
    };
  }, []);

  const recheck = useCallback(() => {
    setChecking(true);
    void read(true).finally(() => setChecking(false));
  }, []);

  return { health, recheck, checking };
}
