import { useEffect, useMemo, type RefObject } from "react";
import { closeHole, openHole, type NativeHole } from "@/lib/shell/nativeHoles";

/** The hole over `slotRef`'s slot (`lib/shell/nativeHoles.ts`), closed on
 *  unmount. It remembers the slot it opened over: the ref is already null by
 *  the time an unmount's cleanup closes it. */
export function useNativeHole(slotRef: RefObject<HTMLElement | null>): NativeHole {
  const hole = useMemo<NativeHole>(() => {
    let slot: HTMLElement | null = null;
    return {
      open: () => {
        if (slot) return;
        slot = slotRef.current;
        if (slot) openHole(slot);
      },
      close: () => {
        if (slot) closeHole(slot);
        slot = null;
      },
    };
  }, [slotRef]);

  useEffect(() => () => hole.close(), [hole]);

  return hole;
}
