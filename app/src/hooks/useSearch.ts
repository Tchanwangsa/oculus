import { useEffect, useRef, useState } from "react";
import { runSearch, type SearchOptions, type SearchSection } from "@/lib/search";

/**
 * A search field's results, shared by ⌘K and the new-tab page. The token check
 * stops a slow, out-of-order result for an older query from landing; the
 * debounce keeps `searchPageText`'s per-search scan from running per letter.
 */
const DEBOUNCE_MS = 80;

export function useSearch(query: string, options: SearchOptions): SearchSection[] {
  const [sections, setSections] = useState<SearchSection[]>([]);
  const latest = useRef("");
  const { subjects, current, noWeb } = options;

  useEffect(() => {
    const token = query;
    latest.current = token;
    const timer = setTimeout(() => {
      runSearch(query, { subjects, current, noWeb })
        .then((s) => {
          if (latest.current === token) setSections(s);
        })
        .catch((e) => console.error("[oculus] search", e));
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query, subjects, current, noWeb]);

  return sections;
}
