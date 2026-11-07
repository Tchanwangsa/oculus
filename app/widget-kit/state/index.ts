/** `oculus/state`: state the parent keeps for this widget, by key. */
import { useCallback, useRef, useSyncExternalStore } from "react";
import { getState, setValue, subscribe } from "./store";

type SetState<T> = (next: T | ((prev: T) => T)) => void;

/**
 * Like `useState`, but kept by the reply: it survives reloads, theme changes
 * and the expanded view. `initial` is used while the key has no saved value;
 * values must be JSON (numbers, strings, booleans, arrays, plain objects).
 */
export function useWidgetState<T>(key: string, initial: T): [T, SetState<T>] {
  // The first `initial` only, so a literal object doesn't change the snapshot every render.
  const fallback = useRef(initial);
  const read = useCallback(
    () => (Object.prototype.hasOwnProperty.call(getState(), key) ? (getState()[key] as T) : fallback.current),
    [key],
  );
  const value = useSyncExternalStore(subscribe, read, read);
  const set = useCallback<SetState<T>>(
    (next) => setValue(key, typeof next === "function" ? (next as (prev: T) => T)(read()) : next),
    [key, read],
  );
  return [value, set];
}
