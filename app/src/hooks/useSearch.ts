import { useEffect, useRef, useState } from "react";
import { runSearch, type SearchOptions, type SearchSection } from "@/lib/search";

/**
 * A search field's results, kept in step with what is in it.
 *
 * Shared by the ⌘K palette and the new-tab page's field, so the two cannot
 * differ in the two things that are easy to get subtly wrong:
 *
 * - **A slow read must not overwrite a newer one.** Five queries go out per
 *   keystroke and they do not come back in order; the token check is what
 *   keeps a result belonging to a query you have already typed past from
 *   landing on screen.
 * - **A keystroke is not a query.** The page-text index is scanned per search
 *   (`searchPageText`), so typing a word at speed would otherwise run it once
 *   per letter for answers nobody reads. A short wait collapses those into
 *   one, and is under the threshold where a list feels like it is lagging the
 *   field.
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
