import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { ArrowClockwise, CaretLeft } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import {
  music,
  playlistSubtitle,
  type MusicPlaylist,
  type MusicSearchResults,
  type MusicSnapshot,
} from "@/lib/music";
import { LibraryRow } from "./LibraryRow";
import { LibrarySearch } from "./LibrarySearch";
import { PlaylistView } from "./PlaylistView";
import { SearchGroups } from "./SearchGroups";

interface Search {
  query: string;
  results: MusicSearchResults | null;
  error: string | null;
}

/** Playlists Spotify gives no picture for, which get one made. */
function coverless(list: MusicPlaylist[]): string[] {
  return list.filter((p) => p.images.length === 0).map((p) => p.uri);
}

/**
 * The player's browser: the account's playlists, one of them opened to its
 * tracks, and a search that replaces them while it has results. All are
 * calls Spotify can rate-limit, so the playlists load once per mount (Rust
 * answers later mounts from memory until the refresh button asks again),
 * covers for playlists without a picture are filled in after the list, and
 * search runs on Enter only.
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
  const searchSeq = useRef(0);
  const asked = useRef(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef, "y", search ? "search" : (openUri ?? "playlists"));

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

  const runSearch = () => {
    const q = query.trim();
    if (!q) return;
    const seq = ++searchSeq.current;
    setSearch({ query: q, results: null, error: null });
    music.search(q).then(
      (results) => {
        if (seq === searchSeq.current) setSearch({ query: q, results, error: null });
      },
      (e: unknown) => {
        if (seq === searchSeq.current) setSearch({ query: q, results: null, error: String(e) });
      },
    );
  };

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

  const clearSearch = () => {
    searchSeq.current++;
    setQuery("");
    setSearch(null);
  };

  const disabled = !snapshot.connected;
  const currentUri = snapshot.track?.uri ?? null;
  const found = search?.results;
  const empty = found && found.playlists.length + found.albums.length + found.tracks.length === 0;
  const opened = (openUri && playlists?.find((p) => p.uri === openUri)) || null;

  return (
    <div className="flex flex-col gap-2">
      <LibrarySearch
        value={query}
        onChange={setQuery}
        onSearch={runSearch}
        onClear={clearSearch}
        disabled={disabled}
      />
      <div className="flex h-6 items-center justify-between px-2">
        {opened && !search ? (
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
          <h2 className="truncate text-xs font-medium text-muted-foreground">
            {search ? `Results for “${search.query}”` : "Your playlists"}
          </h2>
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
        <div ref={scrollRef} className="max-h-96 overflow-y-auto">
          <div>
            {search ? (
              search.error ? (
                <p className="px-2 text-xs text-destructive">{search.error}</p>
              ) : !found ? (
                <SkeletonRows count={4} rowClassName="h-11" className="px-2" />
              ) : empty ? (
                <p className="px-2 text-xs text-muted-foreground">Nothing found.</p>
              ) : (
                <SearchGroups results={found} currentUri={currentUri} disabled={disabled} />
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
      )}
    </div>
  );
}
