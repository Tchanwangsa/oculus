import type { ThreadSearchResult } from "@/lib/harness";
import { SearchRow } from "./SearchRow";
import type { ThreadSearch } from "./useThreadSearch";

/** The column's body while searching: a count line, then one row per chat,
 *  best match first (`searchThreads`); none is one muted line. */
export function SearchResults({
  found,
  activeId,
  onOpen,
}: {
  found: ThreadSearch;
  activeId: number | null;
  onOpen: (result: ThreadSearchResult) => void;
}) {
  const n = found.results.length;
  const q = found.q.trim();
  return (
    <section aria-label="Search results" className="flex flex-col gap-0.5">
      <p className="px-2.5 pt-1 pb-1 text-[11.5px] text-muted-foreground">
        {n === 0 ? (
          <>No chats mention “{q}”</>
        ) : (
          <>
            <span className="tabular-nums">{n}</span> {n === 1 ? "chat mentions" : "chats mention"} “{q}”
          </>
        )}
      </p>
      {found.results.map((r) => (
        <SearchRow key={r.id} result={r} active={r.id === activeId} onOpen={onOpen} />
      ))}
    </section>
  );
}
