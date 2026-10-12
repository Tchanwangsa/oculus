import { useCallback } from "react";

import { readStringSet, useStoredState } from "@/hooks/ui/useStoredState";

const writeSet = (value: Set<string>) => JSON.stringify([...value]);

/**
 * Which sections of an inspector are folded, remembered under `key`. Until
 * the user has folded or opened anything, the `closed` ids start folded and
 * every other id (a section a later version adds) starts open.
 */
export function useInspectorSections(key: string, closed: readonly string[] = []) {
  const [folded, setFolded] = useStoredState(
    key,
    (raw) => (raw == null ? new Set(closed) : readStringSet(raw)),
    writeSet,
  );
  const isOpen = useCallback((id: string) => !folded.has(id), [folded]);
  const setOpen = useCallback(
    (id: string, open: boolean) =>
      setFolded((prev) => {
        const next = new Set(prev);
        if (open) next.delete(id);
        else next.add(id);
        return next;
      }),
    [setFolded],
  );
  return { isOpen, setOpen };
}
