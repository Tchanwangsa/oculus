import { useSyncExternalStore } from "react";
import { mathsReady, onMathsReady } from "./engine";

/** `mathsReady()` for a component that renders maths: it re-renders when the
 *  engine comes up, so maths drawn as a placeholder before then is redrawn.
 *  A `memo` component that runs `rehypeMaths` must call it itself. */
export function useMathsReady(): boolean {
  return useSyncExternalStore(onMathsReady, mathsReady);
}
