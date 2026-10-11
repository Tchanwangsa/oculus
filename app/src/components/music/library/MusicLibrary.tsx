import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { ArrowClockwise, CaretLeft } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { cn } from "@/lib/utils";
import {
  music,
  playlistSubtitle,
  shownPlayback,
  type MusicPlaylist,
  type MusicSearchItem,
  type MusicSearchResults,
  type MusicSnapshot,
} from "@/lib/music";
import { LibraryRow } from "./LibraryRow";
import { LibrarySearch } from "./LibrarySearch";
import { PlaylistView, type OpenedList } from "./PlaylistView";
import { SearchGroups, searchFilters, type SearchFilter } from "./SearchGroups";

interface Search {
  query: string;
  results: MusicSearchResults | null;
  error: string | null;
}

/** Typing searches by itself once the query is longer than three characters
 *  and the typing has paused this long; Enter searches at once, any length. */
const AUTO_MIN_CHARS = 4;
const AUTO_DELAY_MS = 500;

/** Playlists Spotify gives no picture for, which get one made. */
function coverless(list: MusicPlaylist[]): string[] {
  return list.filter((p) => p.images.length === 0).map((p) => p.uri);
}

/**
 * The player's browser: the account's playlists, one of them opened to its
 * tracks, and a search that replaces them while it has results, its albums
 * and playlists opening to their tracks over the results. All are
 * calls Spotify can rate-limit, so the playlists load once per mount (Rust
 * answers later mounts from memory until the refresh button asks again),
 * covers for playlists without a picture are filled in after the list, and
 * search runs on Enter or a pause in typing, never per keystroke.
 */
export function MusicLibrary({ snapshot }: { snapshot: MusicSnapshot }) {
  const [playlists, setPlaylists] = useState<MusicPlaylist[] | null>(null);
  /** The playlist opened to its tracks. */
  const [openUri, setOpenUri] = useState<string | null>(null);
  /** The playlists' scroll position when one was opened, put back on Back. */
  const savedScroll = useRef<number | null>(null);
  const coverSeq = useRef(0);
  const [loading, setLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState<Search | null>(null);
  const [filter, setFilter] = useState<SearchFilter>("all");
  /** An album or playlist opened from the results, which stay mounted under
   *  it so Back finds their place and loaded pages. */
  const [searchOpen, setSearchOpen] = useState<OpenedList | null>(null);
  const searchSeq = useRef(0);
  /** The query last searched, so a pause after Enter doesn't ask again. */
  const searched = useRef<string | null>(null);
  const asked = useRef(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef, "y", search ? `search:${filter}` : (openUri ?? "playlists"));

  // A failure leaves the glyph: Rust logs it, and a rate limit already shows
  // on the next call that needs Spotify.
  const fillCovers = useCallback((list: MusicPlaylist[]) => {
    const missing = coverless(list);
    if (missing.length === 0) return;
    const seq = ++coverSeq.current;
    music.playlistCovers(missing).then(
      (found) => {
        if (seq !== coverSeq.current) return;
        setPlaylists(
          (prev) =>
            prev?.map((p) =>
              p.images.length === 0 && found[p.uri]?.length ? { ...p, images: found[p.uri] } : p,
            ) ?? null,
        );
      },
      () => {},
    );
  }, []);

  const load = useCallback(
    (refresh: boolean) => {
      setLoading(true);
      setListError(null);
      music.playlists(refresh).then(
        (list) => {
          setPlaylists(list);
          setLoading(false);
          fillCovers(list);
        },
        (e: unknown) => {
          setListError(String(e));
          setLoading(false);
        },
      );
    },
    [fillCovers],
  );

  useEffect(() => {
    if (!snapshot.connected || asked.current) return;
    asked.current = true;
    load(false);
  }, [snapshot.connected, load]);

  const runSearch = useCallback((raw: string) => {
    const q = raw.trim();
    if (!q) return;
    searched.current = q;
    const seq = ++searchSeq.current;
    setSearch({ query: q, results: null, error: null });
    setFilter("all");
    setSearchOpen(null);
    music.search(q).then(
      (results) => {
        if (seq === searchSeq.current) setSearch({ query: q, results, error: null });
      },
      (e: unknown) => {
        if (seq === searchSeq.current) setSearch({ query: q, results: null, error: String(e) });
      },
    );
  }, []);

  const disabled = !snapshot.connected;

  useEffect(() => {
    const q = query.trim();
    if (disabled || q.length < AUTO_MIN_CHARS) return;
    const timer = window.setTimeout(() => {
      if (q !== searched.current) runSearch(q);
    }, AUTO_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [query, disabled, runSearch]);

  const openPlaylist = (uri: string) => {
    savedScroll.current = scrollRef.current?.scrollTop ?? 0;
    setOpenUri(uri);
  };

  // The playlists' scroller is mounted again on Back: put its place back.
  useLayoutEffect(() => {
    if (openUri !== null || search || savedScroll.current === null || !scrollRef.current) return;
    scrollRef.current.scrollTop = savedScroll.current;
    savedScroll.current = null;
  }, [openUri, search]);

  // Another filter starts at the top of its list.
  useLayoutEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = 0;
  }, [filter]);

  const clearSearch = () => {
    searchSeq.current++;
    searched.current = null;
    setQuery("");
    setSearch(null);
    setSearchOpen(null);
  };

  // Emptying the field by hand clears the results too, so an error is never
  // left with no × to dismiss it.
  const changeQuery = (value: string) => {
    if (value.trim()) setQuery(value);
    else clearSearch();
  };

  const currentUri = shownPlayback(snapshot).track?.uri ?? null;
  const found = search?.results;
  const filters = found ? searchFilters(found) : [];
  // "All" alone means nothing was found.
  const empty = found && filters.length === 1;
  const own = (openUri && playlists?.find((p) => p.uri === openUri)) || null;
  const opened: OpenedList | null = own && {
    uri: own.uri,
    name: own.name,
    subtitle: playlistSubtitle(own),
    images: own.images,
  };
  const fromSearch = search ? searchOpen : null;
  const openFromSearch = (item: MusicSearchItem) =>
    setSearchOpen({
      uri: item.uri,
      name: item.name,
      subtitle: item.subtitle,
      images: item.image_url ? [item.image_url] : [],
    });

  return (
    <div className="flex flex-col gap-2 @3xl:min-h-0 @3xl:flex-1">
      <LibrarySearch
        value={query}
        onChange={changeQuery}
        onSearch={() => runSearch(query)}
        onClear={clearSearch}
        disabled={disabled}
      />
      <div className="flex h-6 items-center justify-between gap-2 px-2">
        {fromSearch ? (
          <Button
            variant="ghost"
            size="xs"
            className="-ml-2 min-w-0 text-muted-foreground"
            onClick={() => setSearchOpen(null)}
          >
            <CaretLeft />
            <span className="truncate">Results for “{search?.query}”</span>
          </Button>
        ) : opened && !search ? (
          <Button
            variant="ghost"
            size="xs"
            className="-ml-2 text-muted-foreground"
            onClick={() => setOpenUri(null)}
          >
            <CaretLeft />
            Your playlists
          </Button>
        ) : (
          <h2 className="min-w-0 truncate text-xs font-medium text-muted-foreground">
            {search ? `Results for “${search.query}”` : "Your playlists"}
          </h2>
        )}
        {search && !fromSearch && filters.length > 2 && (
          <PillTabs tabs={filters} value={filter} onChange={setFilter} className="shrink-0" />
        )}
        {!search && !opened && (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Refresh playlists"
            disabled={disabled || loading}
            onClick={() => load(true)}
          >
            <ArrowClockwise size={12} />
          </Button>
        )}
      </div>
      {opened && !search ? (
        <PlaylistView playlist={opened} currentUri={currentUri} disabled={disabled} />
      ) : (
        <>
          {fromSearch && (
            <PlaylistView
              key={fromSearch.uri}
              playlist={fromSearch}
              currentUri={currentUri}
              disabled={disabled}
            />
          )}
          <div
            ref={scrollRef}
            className={cn(
              "max-h-96 overflow-y-auto @3xl:max-h-none @3xl:min-h-0 @3xl:flex-1",
              fromSearch && "hidden",
            )}
          >
            <div>
              {search ? (
                search.error ? (
                  <p className="px-2 text-xs text-destructive">{search.error}</p>
                ) : !found ? (
                  <SkeletonRows count={4} rowClassName="h-11" className="px-2" />
                ) : empty ? (
                  <p className="px-2 text-xs text-muted-foreground">Nothing found.</p>
                ) : (
                  <SearchGroups
                    query={search.query}
                    results={found}
                    filter={filter}
                    onFilter={setFilter}
                    onOpen={openFromSearch}
                    currentUri={currentUri}
                    disabled={disabled}
                  />
                )
              ) : (
                <>
                  {listError && <p className="px-2 pb-1 text-xs text-destructive">{listError}</p>}
                  {playlists === null ? (
                    !listError && <SkeletonRows count={4} rowClassName="h-11" className="px-2" />
                  ) : playlists.length === 0 ? (
                    <p className="px-2 text-xs text-muted-foreground">
                      No playlists in this account.
                    </p>
                  ) : (
                    playlists.map((p) => (
                      <LibraryRow
                        key={p.uri}
                        name={p.name}
                        subtitle={playlistSubtitle(p)}
                        images={p.images}
                        onPlay={() => openPlaylist(p.uri)}
                      />
                    ))
                  )}
                </>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
