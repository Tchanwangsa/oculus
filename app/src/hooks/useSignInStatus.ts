import { useCallback, useEffect, useState } from "react";

import {
  harnessSignInStatus,
  PROVIDERS,
  type Provider,
  type SignInStatus,
} from "@/lib/harness";

/**
 * Whether each CLI is signed in — one answer, shared by the three places that
 * ask: Settings → AI, the composer's warning line, and the sign-in dialog's
 * end state.
 *
 * Shaped after `app/src/hooks/useBridgeHealth.ts` deliberately, because it is
 * the same kind of value: one fact per provider that arrives once, is read
 * wherever a turn might be sent, and is edited by nobody. So it is a
 * module-level cache and a single in-flight promise rather than a store —
 * three surfaces mounting at once make one round of probes, not three.
 *
 * **One difference matters.** Rust caches health (`discover.rs` memoises the
 * lookup and the `--version` behind it); it caches *nothing* here, because
 * answering means spawning the CLI and asking it. So this cache is the only
 * one there is, which cuts both ways: a probe is expensive enough to be worth
 * holding, and a sign-in run that just succeeded has to drop it or every
 * surface goes on saying "signed out". `recheck` is that drop, and
 * `useSignIn`'s `onFinished` is its caller.
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
  // A probe that could not run is not evidence that the student is signed
  // out — the same rule health follows. A rejected invoke drops that provider
  // from the answer entirely, which reads as `unknown` everywhere, rather
  // than flashing the one state that puts a Sign in button on screen.
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

/**
 * A provider's state, given whatever has landed so far.
 *
 * Three states for the same reason `providerHealth` has three: until the probe
 * returns, a provider is neither signed in nor signed out, and every surface
 * draws `unknown` as *nothing at all*. An `error` row is `unknown` too — the
 * probe failing says nothing about the credential — and so is opencode's
 * `signedIn: null`, which is the deliberate "not answerable from here".
 */
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
  /** Null until the first round of probes lands. */
  statuses: SignInStatus[] | null;
  /** Drop the cache and ask every CLI again. Settings' *Recheck*, and the end
   *  of a sign-in run. */
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
