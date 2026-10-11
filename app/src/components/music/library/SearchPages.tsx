import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { music, type MusicSearchItem, type MusicSearchKind } from "@/lib/music";

/** Hits per group in `music_search`'s answer, so the first page to ask for
 *  starts here. */
const FIRST_PAGE = 10;

/**
 * One search group, all of it: the hits `music_search` found, then a page
 * of `music_search_more` each time the end of the list scrolls into view,
 * until Spotify names no next page. Mounted per query and group, so a new
 * search or filter starts over. A failed page shows its error with Try
 * again rather than asking on its own.
 */
export function SearchPages({
  query,
  kind,
  first,
  renderRow,
}: {
  query: string;
  kind: MusicSearchKind;
  first: MusicSearchItem[];
  renderRow: (item: MusicSearchItem) => ReactNode;
}) {
  const [items, setItems] = useState(first);
  const [next, setNext] = useState<number | null>(first.length > 0 ? FIRST_PAGE : null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const end = useRef<HTMLDivElement>(null);
  const live = useRef(true);

  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);

  const loadMore = useCallback(() => {
    if (next === null) return;
    setLoading(true);
    music.searchMore(query, kind, next).then(
      (page) => {
        if (!live.current) return;
        setItems((prev) => {
          const seen = new Set(prev.map((i) => i.uri));
          return [...prev, ...page.items.filter((i) => !seen.has(i.uri))];
        });
        setNext(page.items.length > 0 ? page.next_offset : null);
        setLoading(false);
      },
      (e: unknown) => {
        if (!live.current) return;
        setError(String(e));
        setLoading(false);
      },
    );
  }, [query, kind, next]);

  // Re-observed after each page, so an end still in view asks for the next.
  useEffect(() => {
    const el = end.current;
    if (!el || next === null || loading || error) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) loadMore();
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [next, loading, error, loadMore]);

  return (
    <div>
      {items.map(renderRow)}
      {loading && <SkeletonRows count={2} rowClassName="h-11" className="px-2" />}
      {error && (
        <div className="flex items-center gap-2 px-2 py-1">
          <p className="min-w-0 flex-1 text-xs text-destructive">{error}</p>
          <Button variant="ghost" size="xs" onClick={() => setError(null)}>
            Try again
          </Button>
        </div>
      )}
      <div ref={end} className="h-px" />
    </div>
  );
}
