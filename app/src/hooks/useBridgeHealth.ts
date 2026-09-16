import { useCallback, useEffect, useState } from "react";

import { harnessHealth, type BridgeHealth, type Provider } from "@/lib/harness";

/**
 * Whether a provider's CLI is on this machine — one answer, shared by every
 * picker in the app.
 *
 * This used to be the Settings page's own state, which is why a student with
 * no Claude Code installed still browsed the whole Claude catalogue in the
 * composer and only found out at send time. Health is now read wherever a
 * model is chosen, so it is read *often*: a picker mounts in both composers
 * and in every job row of Settings → AI.
 *
 * Two caches keep that honest. Rust memoises the lookup and the `--version`
 * behind it (`app/src-tauri/src/harness/discover.rs`) and only Settings'
 * *Recheck* clears them, since a full recheck can end in a login shell's
 * `command -v` per provider. And this module holds the answer plus the
 * in-flight promise, so ten pickers mounting at once make one invoke rather
 * than ten — a module-level cache rather than a store, because nothing here
 * is edited: it is one value that arrives once and changes only when Settings
 * asks it to.
 */
export type ProviderHealth = "unknown" | "installed" | "missing";

let cached: BridgeHealth[] | null = null;
let inFlight: Promise<BridgeHealth[]> | null = null;
const listeners = new Set<(rows: BridgeHealth[]) => void>();

function read(recheck: boolean): Promise<BridgeHealth[]> {
  if (!recheck) {
    if (cached) return Promise.resolve(cached);
    if (inFlight) return inFlight;
  }
  // A failed invoke answers with an empty list, which reads as `unknown`
  // everywhere: a health check that could not run is not evidence that a CLI
  // is absent, and claiming so would be the wrong state to flash.
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

/** A provider's state, given whatever health has landed so far. `unknown` is
 *  a real answer and the important one: until the check returns, a provider is
 *  neither installed nor missing, and the picker draws it as it always did. */
export function providerHealth(rows: BridgeHealth[] | null, provider: Provider): ProviderHealth {
  if (!rows) return "unknown";
  const row = rows.find((h) => h.provider === provider);
  if (!row) return "unknown";
  // A path that was found but will not run (`--version` failed) still counts
  // as installed — Settings shows that error in full, and the picker's job is
  // the one distinction a student can act on from a menu.
  return row.path ? "installed" : "missing";
}

export function useBridgeHealth(): {
  /** Null until the first answer lands. */
  health: BridgeHealth[] | null;
  /** A full recheck: Rust forgets where it found things and looks again.
   *  Settings' button, and nothing else. */
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
