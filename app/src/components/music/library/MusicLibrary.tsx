import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowClockwise } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import {
  music,
  type MusicPlaylist,
  type MusicSearchResults,
  type MusicSnapshot,
} from "@/lib/music";
import { useMusicStore } from "@/stores/music/musicStore";
import { LibraryRow } from "./LibraryRow";
import { LibrarySearch } from "./LibrarySearch";
import { SearchGroups } from "./SearchGroups";

function playlistSubtitle(p: MusicPlaylist): string {
  const count =
    p.track_count === null ? null : `${p.track_count} ${p.track_count === 1 ? "track" : "tracks"}`;
  return [p.owner, count].filter(Boolean).join(" · ");
}

interface Search {
  query: string;
  results: MusicSearchResults | null;
  error: string | null;
}

/**
 * The player's browser: the account's playlists, and a search that replaces
 * them while it has results. Both are Web API calls Spotify rate-limits, so
 * the playlists load once per mount (Rust answers later mounts from memory
 * until the refresh button asks again) and search runs on Enter only.
 */
export function MusicLibrary({ snapshot }: { snapshot: MusicSnapshot }) {
  const run = useMusicStore((s) => s.run);
  const [playlists, setPlaylists] = useState<MusicPlaylist[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState<Search | null>(null);
  const searchSeq = useRef(0);
  const asked = useRef(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef, "y", search ? "search" : "playlists");

  const load = useCallback((refresh: boolean) => {
    setLoading(true);
    setListError(null);
    music.playlists(refresh).then(
      (list) => {
        setPlaylists(list);
        setLoading(false);
      },
      (e: unknown) => {
        setListError(String(e));
        setLoading(false);
      },
    );
  }, []);

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

  const clearSearch = () => {
    searchSeq.current++;
    setQuery("");
    setSearch(null);
  };

  const disabled = !snapshot.connected;
  const currentUri = snapshot.track?.uri ?? null;
  const found = search?.results;
  const empty =
    found && found.playlists.length + found.albums.length + found.tracks.length === 0;

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
        <h2 className="truncate text-xs font-medium text-muted-foreground">
          {search ? `Results for “${search.query}”` : "Your playlists"}
        </h2>
        {!search && (
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
                <p className="px-2 text-xs text-muted-foreground">No playlists in this account.</p>
              ) : (
                playlists.map((p) => (
                  <LibraryRow
                    key={p.uri}
                    name={p.name}
                    subtitle={playlistSubtitle(p)}
                    imageUrl={p.image_url}
                    disabled={disabled}
                    onPlay={() => run(() => music.playContext(p.uri))}
                  />
                ))
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
