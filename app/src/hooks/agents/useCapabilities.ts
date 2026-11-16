import { useMemo, useSyncExternalStore } from "react";
import { capabilities, PROVIDER_IDS, switchLocked, type Capabilities, type HarnessRun, type Provider } from "@/lib/harness";

/** What `provider`'s bridge can do, from Rust (`harness_capabilities`). Until
 *  the answer lands every optional action reads as unavailable. */
export function useCapabilities(provider: Provider): Capabilities {
  const read = () => capabilities.of(provider);
  // The same read on a server render (a test's), which has no store to wait for.
  return useSyncExternalStore(capabilities.subscribe, read, read);
}

/** Whether a run's bridge steers only by stopping and starting again
 *  (`steerByRestart` without `activeSteering`), as one test that stays the
 *  same object until the answer changes, for `buildRows`. */
export function useRestartSteering(): (run: HarnessRun) => boolean {
  const read = () =>
    PROVIDER_IDS.filter((p) => {
      const c = capabilities.of(p);
      return c.steerByRestart && !c.activeSteering;
    }).join(" ");
  const key = useSyncExternalStore(capabilities.subscribe, read, read);
  return useMemo(() => {
    const restarts = new Set(key.split(" "));
    return (run: HarnessRun) => !!run.provider && restarts.has(run.provider);
  }, [key]);
}

/** Which providers a chat on `current` cannot move to (`switchLocked`), as a
 *  test that stays the same object until the answer changes, so a memoised
 *  picker is not redrawn. `current` null is a chat not started: none. */
export function useSwitchLock(current: Provider | null): (provider: Provider) => boolean {
  const read = () => PROVIDER_IDS.filter((p) => switchLocked(current, p, capabilities.of)).join(" ");
  const key = useSyncExternalStore(capabilities.subscribe, read, read);
  return useMemo(() => {
    const locked = new Set(key.split(" "));
    return (provider: Provider) => locked.has(provider);
  }, [key]);
}
