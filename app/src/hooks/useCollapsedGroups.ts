import { useStoredSet } from "./useStoredState";

/** Stores collapsed keys, so newly discovered groups arrive open. */
export function useCollapsedGroups(storageKey: string) {
  const [folded, setFolded] = useStoredSet(storageKey);
  const setOpen = (key: string, open: boolean) => setFolded((prev) => {
    if (open === !prev.has(key)) return prev;
    const next = new Set(prev);
    if (open) next.delete(key);
    else next.add(key);
    return next;
  });
  return { folded, setOpen };
}
