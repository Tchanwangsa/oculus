import { useMemo, useState } from "react";

/** Search rows keep their source indexes, so measured heights survive filtering. */
export function useTranscriptSearch(items: readonly { text: string }[], activeIndex: number) {
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const searching = needle.length > 0;
  const rows = useMemo(() => {
    const out: number[] = [];
    for (let i = 0; i < items.length; i++) {
      if (!needle || items[i].text.toLowerCase().includes(needle)) out.push(i);
    }
    return out;
  }, [items, needle]);
  // The playing item may be missing from results; searching suspends following.
  return { query, setQuery, needle, searching, rows, followIdx: searching ? -1 : activeIndex };
}
