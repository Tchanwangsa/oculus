import { music, type MusicSearchItem, type MusicSearchResults } from "@/lib/music";
import { useMusicStore } from "@/stores/music/musicStore";
import { LibraryRow } from "./LibraryRow";

const GROUPS = [
  { key: "playlists", label: "Playlists" },
  { key: "albums", label: "Albums" },
  { key: "tracks", label: "Tracks" },
] as const;

/** Search results under Playlists, Albums and Tracks. A playlist or album
 *  plays as a context; a track plays from its place in its album, and can
 *  be added to the queue. */
export function SearchGroups({
  results,
  currentUri,
  disabled,
}: {
  results: MusicSearchResults;
  currentUri: string | null;
  disabled: boolean;
}) {
  const run = useMusicStore((s) => s.run);
  const play = (item: MusicSearchItem) =>
    void run(() =>
      item.uri.startsWith("spotify:track:")
        ? music.playTrack(item.uri, item.context_uri)
        : music.playContext(item.uri),
    );

  return (
    <div className="flex flex-col gap-3">
      {GROUPS.map(({ key, label }) =>
        results[key].length === 0 ? null : (
          <section key={key}>
            <h3 className="px-2 pb-1 text-xs font-medium text-muted-foreground">{label}</h3>
            {results[key].map((item) => (
              <LibraryRow
                key={item.uri}
                name={item.name}
                subtitle={item.subtitle}
                images={item.image_url ? [item.image_url] : []}
                queueUri={key === "tracks" ? item.uri : undefined}
                current={item.uri === currentUri}
                disabled={disabled}
                onPlay={() => play(item)}
              />
            ))}
          </section>
        ),
      )}
    </div>
  );
}
