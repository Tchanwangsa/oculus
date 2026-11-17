import { useLayoutEffect, useSyncExternalStore } from "react";
import { subscribeWarp, warpLanding } from "@/lib/ui/warp";

/**
 * Whether a warp (`lib/ui/warp.ts`) lands on this pane's composer for
 * `thread`; while true the caller gives the composer `WARP_NAME`. Tells the
 * warp once that render has committed, so the new snapshot holds it.
 */
export function useWarpLanding(pane: number, thread: number | null): boolean {
  const landing = useSyncExternalStore(subscribeWarp, warpLanding, () => null);
  const here = landing != null && thread != null && landing.pane === pane && landing.thread === thread;
  useLayoutEffect(() => {
    if (here) landing.land();
  }, [here, landing]);
  return here;
}
