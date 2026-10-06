import { useMemo, useState } from "react";

/** Search rows keep their source indexes, so measured heights survive filtering. */
export function useTranscriptSearch(items: readonly { text: string }[], activeIndex: number) {
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const searching = needle.length > 0;
  const normalized = useMemo(
    () => items.map((item, index) => ({ index, text: item.text.toLowerCase() })),
    [items],
  );
  const rows = useMemo(() => {
    const out: number[] = [];
    for (const item of normalized) {
      if (!needle || item.text.includes(needle)) out.push(item.index);
    }
    return out;
  }, [normalized, needle]);
  // The playing item may be missing from results; searching suspends following.
  return { query, setQuery, needle, searching, rows, followIdx: searching ? -1 : activeIndex };
}

export type TranscriptSearch = ReturnType<typeof useTranscriptSearch>;
