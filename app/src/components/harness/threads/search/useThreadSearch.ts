import { useEffect, useMemo, useState } from "react";
import { debouncedSearch, searchThreads, type ThreadSearchResult } from "@/lib/harness";

export interface ThreadSearch {
  /** The query these results answer. */
  q: string;
  results: ThreadSearchResult[];
}

/** The column's search for `query`, debounced (`debouncedSearch`): the
 *  latest answer, held while the next is on its way; null once cleared. */
export function useThreadSearch(query: string): ThreadSearch | null {
  const [found, setFound] = useState<ThreadSearch | null>(null);
  const search = useMemo(
    () => debouncedSearch(searchThreads, (results, q) => setFound(results == null ? null : { q, results })),
    [],
  );
  useEffect(() => search.set(query), [search, query]);
  useEffect(() => () => search.dispose(), [search]);
  return found;
}
