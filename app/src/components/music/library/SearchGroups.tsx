import { Button } from "@/components/ui/button";
import {
  music,
  type MusicSearchItem,
  type MusicSearchKind,
  type MusicSearchResults,
} from "@/lib/music";
import { useMusicStore } from "@/stores/music/musicStore";
import { LibraryRow } from "./LibraryRow";
import { SearchPages } from "./SearchPages";

const GROUPS = [
  { key: "tracks", label: "Songs" },
  { key: "albums", label: "Albums" },
  { key: "playlists", label: "Playlists" },
] as const satisfies ReadonlyArray<{ key: MusicSearchKind; label: string }>;

export type SearchFilter = "all" | MusicSearchKind;

/** Rows each group shows under All before its "Show all". */
const PREVIEW = 4;

/** The filter chips: All, then each group that found something. */
export function searchFilters(
  results: MusicSearchResults,
): { value: SearchFilter; label: string }[] {
  return [
    { value: "all", label: "All" },
    ...GROUPS.filter(({ key }) => results[key].length > 0).map(({ key, label }) => ({
      value: key,
      label,
    })),
  ];
}

/** Search results: under All, a few of each group under its heading, Songs
 *  first; under one group's filter, all of it, loading more as it scrolls.
 *  A track plays from its place in its album, and can be added to the
 *  queue; an album or playlist opens to its tracks. */
export function SearchGroups({
  query,
  results,
  filter,
  onFilter,
  onOpen,
  currentUri,
  disabled,
}: {
  query: string;
  results: MusicSearchResults;
  filter: SearchFilter;
  onFilter: (filter: SearchFilter) => void;
  onOpen: (item: MusicSearchItem) => void;
  currentUri: string | null;
  disabled: boolean;
}) {
  const run = useMusicStore((s) => s.run);

  const row = (kind: MusicSearchKind, item: MusicSearchItem) => (
    <LibraryRow
      key={item.uri}
      name={item.name}
      subtitle={item.subtitle}
      images={item.image_url ? [item.image_url] : []}
      queueUri={kind === "tracks" ? item.uri : undefined}
      current={item.uri === currentUri}
      disabled={kind === "tracks" && disabled}
      onPlay={() =>
        kind === "tracks"
          ? void run(() => music.playTrack(item.uri, item.context_uri))
          : onOpen(item)
      }
    />
  );

  if (filter !== "all") {
    return (
      <SearchPages
        key={`${query}:${filter}`}
        query={query}
        kind={filter}
        first={results[filter]}
        renderRow={(item) => row(filter, item)}
      />
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {GROUPS.map(({ key, label }) =>
        results[key].length === 0 ? null : (
          <section key={key}>
            <div className="flex h-7 items-center justify-between px-2">
              <h3 className="text-sm font-semibold text-foreground">{label}</h3>
              {results[key].length > PREVIEW && (
                <Button
                  variant="ghost"
                  size="xs"
                  className="-mr-2 text-muted-foreground"
                  onClick={() => onFilter(key)}
                >
                  Show all
                </Button>
              )}
            </div>
            {results[key].slice(0, PREVIEW).map((item) => row(key, item))}
          </section>
        ),
      )}
    </div>
  );
}
