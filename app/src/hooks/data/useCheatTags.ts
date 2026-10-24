import { useCallback, useEffect, useRef, useState } from "react";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { getCheatTags, REVISION_UPDATED_EVENT, type CheatTag } from "@/lib/revision";

/** The cheat sheet's tag registry, re-read on `REVISION_UPDATED_EVENT` (an
 *  entry saved with a new tag, or a card's Add). */
export function useCheatTags(): CheatTag[] {
  const [tags, setTags] = useState<CheatTag[]>([]);
  // Only the newest read sets state when a burst of writes lands out of order.
  const run = useRef(0);
  const load = useCallback(() => {
    const token = ++run.current;
    getCheatTags()
      .then((rows) => {
        if (run.current === token) setTags(rows);
      })
      .catch((e) => console.error("read cheat tags failed", e));
  }, []);
  useEffect(load, [load]);
  useWindowEvent(REVISION_UPDATED_EVENT, load);
  return tags;
}
