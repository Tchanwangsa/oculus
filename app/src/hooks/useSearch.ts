import { useEffect, useRef, useState } from "react";
import { runSearch, type SearchOptions, type SearchSection } from "@/lib/search";

/**
 * A search field's results, shared by ⌘K and the new-tab page. The token check
 * stops a slow, out-of-order result for an older search from landing; the
 * debounce keeps `searchPageText`'s per-search scan from running per letter.
 * `filters` and `draft` must be stable (state or memo), or every render searches.
 */
const DEBOUNCE_MS = 80;

export function useSearch(query: string, options: SearchOptions): SearchSection[] {
  const [sections, setSections] = useState<SearchSection[]>([]);
  const latest = useRef<symbol>(undefined);
  const { subjects, current, noWeb, filters, offerFilters, draft } = options;

  useEffect(() => {
    // Per run, not per query: a chip changes the results, not the text.
    const token = Symbol();
    latest.current = token;
    const timer = setTimeout(() => {
      runSearch(query, { subjects, current, noWeb, filters, offerFilters, draft })
        .then((s) => {
          if (latest.current === token) setSections(s);
        })
        .catch((e) => console.error("[oculus] search", e));
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, subjects, current, noWeb, filters, offerFilters, draft]);

  return sections;
}
