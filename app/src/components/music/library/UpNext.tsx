import { useCallback, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useContextRest } from "@/hooks/music/useContextRest";
import { useTrackRows } from "@/hooks/music/useTrackRows";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { music, shownPlayback, trackPages, type MusicSnapshot } from "@/lib/music";
import { useMusicStore } from "@/stores/music/musicStore";
import { TrackRow } from "./TrackRow";

/** A row's height: `LibraryRow`'s 36px cover and its padding. */
const ROW = 48;
/** A Queued or Then label, with the gap above it. */
const LABEL = 28;

/** One line of the list: a label, an `up_next` entry, or a track of the
 *  context after Spotify's own next tracks. `row` is its place in `uris`. */
type Item =
  | { kind: "label"; text: string }
  | { kind: "next"; uri: string; index: number; row: number }
  | { kind: "context"; uri: string; row: number };

/** A key that changes with the line, so the virtualizer recomputes its
 *  offsets when labels or rows move without the count changing. */
function itemKey(item: Item): string {
  if (item.kind === "label") return `label-${item.text}`;
  if (item.kind === "next") return `next-${item.uri}-${item.index}`;
  return `context-${item.row}-${item.uri}`;
}

/**
 * What plays next, from the snapshot's `up_next`: queued tracks, then the
 * rest of the context Spotify lists, then the rest of the playing playlist
 * or album after it (`useContextRest`), as Spotify's desktop queue shows it.
 * Queued and Then label the two only while there are queued tracks. A
 * virtual list, since a playlist runs to thousands: rows are fetched a page
 * at a time as they scroll into view. An `up_next` row plays on a click
 * (`music.skipTo`) and offers Remove on hover; a context row plays from its
 * place in the context and has no Remove, since Spotify does not list it.
 * Shown in the player's queue view, where it fills the column like the
 * library it replaces.
 */
export function UpNext({ snapshot }: { snapshot: MusicSnapshot }) {
  const run = useMusicStore((s) => s.run);
  const next = snapshot.up_next;
  const key = next.map((t) => t.uri).join("|");
  const playback = shownPlayback(snapshot);
  const contextUri = snapshot.context_uri;
  const rest = useContextRest(
    contextUri,
    playback.shuffle,
    next,
    playback.track?.uri ?? null,
  );
  // A stable list while the URIs stay the same: each snapshot is a new array.
  const uris = useMemo(
    () => [...next.map((t) => t.uri), ...rest],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [key, rest],
  );
  // Rows removed here, hidden until `up_next` changes.
  const [removed, setRemoved] = useState<{ key: string; ids: Set<string> }>({
    key,
    ids: new Set(),
  });
  const hidden = removed.key === key ? removed.ids : null;

  const items = useMemo(() => {
    const id = (uri: string, index: number) => `${uri}-${index}`;
    const entries = next
      .map((t, index) => ({ ...t, index }))
      .filter((t) => !hidden?.has(id(t.uri, t.index)));
    const queued = entries.filter((t) => t.queued);
    const then = entries.filter((t) => !t.queued);
    const list: Item[] = [];
    const add = (t: (typeof entries)[number]) =>
      list.push({ kind: "next", uri: t.uri, index: t.index, row: t.index });
    if (queued.length > 0) list.push({ kind: "label", text: "Queued" });
    queued.forEach(add);
    if (queued.length > 0 && then.length + rest.length > 0) {
      list.push({ kind: "label", text: "Then" });
    }
    then.forEach(add);
    rest.forEach((uri, i) => list.push({ kind: "context", uri, row: next.length + i }));
    return list;
    // `key` stands for `next`, which is a new array on every snapshot.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, rest, hidden]);

  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef, "y", items.length);
  const getItemKey = useCallback((i: number) => itemKey(items[i]), [items]);
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => (items[i].kind === "label" ? LABEL : ROW),
    getItemKey,
    overscan: 10,
  });
  const shown = virtualizer.getVirtualItems();
  let first = Infinity;
  let last = -1;
  for (const v of shown) {
    const item = items[v.index];
    if (item && item.kind !== "label") {
      first = Math.min(first, item.row);
      last = Math.max(last, item.row);
    }
  }
  const pages = useMemo(() => (last < 0 ? [] : trackPages(first, last)), [first, last]);
  const rows = useTrackRows(uris, pages);

  const remove = (uri: string, index: number) =>
    void run(() => music.removeFromQueue(uri, index)).then((ok) => {
      if (!ok) return;
      setRemoved((r) => {
        const ids = new Set(r.key === key ? r.ids : []);
        ids.add(`${uri}-${index}`);
        return { key, ids };
      });
    });

  const line = (item: Item) => {
    if (item.kind === "label") {
      return (
        <h3 className="flex h-full items-end px-2 pb-1 text-xs font-medium text-muted-foreground">
          {item.text}
        </h3>
      );
    }
    const { uri } = item;
    if (item.kind === "next") {
      const { index } = item;
      return (
        <TrackRow
          uri={uri}
          row={rows[uri]}
          current={false}
          disabled={false}
          onPlay={() => void run(() => music.skipTo(uri, index))}
          onRemove={() => remove(uri, index)}
        />
      );
    }
    return (
      <TrackRow
        uri={uri}
        row={rows[uri]}
        current={false}
        disabled={false}
        onPlay={() => void run(() => music.playTrack(uri, contextUri))}
      />
    );
  };

  return (
    <div className="flex flex-col gap-2 @3xl:min-h-0 @3xl:flex-1">
      <h2 className="flex h-6 items-center px-2 text-xs font-medium text-muted-foreground">
        Up next
      </h2>
      {items.length === 0 ? (
        <p className="px-2 text-xs text-muted-foreground">Nothing up next.</p>
      ) : (
        <div
          ref={scrollRef}
          className="max-h-96 overflow-y-auto @3xl:max-h-none @3xl:min-h-0 @3xl:flex-1"
        >
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {shown.map((v) => {
              const item = items[v.index];
              if (!item) return null;
              return (
                <div
                  key={v.key}
                  className="absolute inset-x-0"
                  style={{ top: v.start, height: v.size }}
                >
                  {line(item)}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
