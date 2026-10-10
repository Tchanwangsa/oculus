import { useCallback, useEffect, useState } from "react";

import {
  harnessSignInStatus,
  PROVIDERS,
  type Provider,
  type SignInStatus,
} from "@/lib/harness";

/**
 * Whether each CLI is signed in, shared by Settings → Agents, the composer and the
 * sign-in dialog. A module-level cache with one in-flight promise, like
 * `useBridgeHealth`. Rust caches nothing (each check spawns the CLI), so a
 * finished sign-in must call `recheck` or every surface keeps saying "out".
 */
export type SignInState = "unknown" | "in" | "out";

let cached: SignInStatus[] | null = null;
let inFlight: Promise<SignInStatus[]> | null = null;
const listeners = new Set<(rows: SignInStatus[]) => void>();

function read(recheck: boolean): Promise<SignInStatus[]> {
  if (!recheck) {
    if (cached) return Promise.resolve(cached);
    if (inFlight) return inFlight;
  }
  // A check that could not run is not evidence of "signed out": drop that
  // provider so it reads as `unknown`, not as a Sign in button.
  const p = Promise.all(
    PROVIDERS.map((p) => harnessSignInStatus(p.id).catch(() => null)),
  )
    .then((rows) => {
      const found = rows.filter((r): r is SignInStatus => r !== null);
      cached = found;
      inFlight = null;
      for (const l of listeners) l(found);
      return found;
    });
  inFlight = p;
  return p;
}

/** The shared answer outside React (the first-run gate), filling the same
 *  cache the hook reads. */
export function loadSignInStatus(): Promise<SignInStatus[]> {
  return read(false);
}

/** `unknown` (drawn as nothing) until the check lands, and for an error row or
 *  opencode's `signedIn: null`, neither of which says anything about the
 *  credential. */
export function signInState(rows: SignInStatus[] | null, provider: Provider): SignInState {
  if (!rows) return "unknown";
  const row = rows.find((s) => s.provider === provider);
  if (!row || row.signedIn == null) return "unknown";
  return row.signedIn ? "in" : "out";
}

/** What the CLI calls the account, for a provider that is signed in. */
export function signInAccount(rows: SignInStatus[] | null, provider: Provider): string | null {
  return rows?.find((s) => s.provider === provider)?.account ?? null;
}

export function useSignInStatus(): {
  /** Null until the first round of checks lands. */
  statuses: SignInStatus[] | null;
  /** Drop the cache and ask every CLI again. */
  recheck: () => void;
  checking: boolean;
} {
  const [statuses, setStatuses] = useState<SignInStatus[] | null>(cached);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    listeners.add(setStatuses);
    let live = true;
    void read(false).then((rows) => {
      if (live) setStatuses(rows);
    });
    return () => {
      live = false;
      listeners.delete(setStatuses);
    };
  }, []);

  const recheck = useCallback(() => {
    setChecking(true);
    void read(true).finally(() => setChecking(false));
  }, []);

  return { statuses, recheck, checking };
}
