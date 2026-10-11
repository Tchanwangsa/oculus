import type { ReactNode } from "react";
import { fmtClockSecs } from "@/lib/lectures/media";
import { cn } from "@/lib/utils";
import { PlaylistCover } from "./PlaylistCover";
import { QueueAction } from "./QueueAction";

/**
 * One playlist, album or track in the player's lists: a click plays it.
 * A track row can carry its duration and an "Add to queue" action; a row
 * with no `onPlay` (up next) is not a control at all.
 */
export function LibraryRow({
  name,
  subtitle,
  images,
  durationMs,
  queueUri,
  current = false,
  disabled = false,
  onPlay,
}: {
  name: string;
  subtitle: string;
  /** One cover, four for a grid, or none for the glyph. */
  images: string[];
  durationMs?: number;
  /** A track's URI, to offer adding it to the queue. */
  queueUri?: string;
  /** The track playing now. */
  current?: boolean;
  disabled?: boolean;
  onPlay?: () => void;
}) {
  const body: ReactNode = (
    <>
      <PlaylistCover images={images} className="size-9 rounded-md" iconSize={15} />
      <span className="min-w-0 flex-1">
        <span
          className={cn(
            "block truncate text-[13px] font-medium",
            current ? "text-brand" : "text-foreground",
          )}
        >
          {name}
        </span>
        {subtitle && (
          <span className="block truncate text-xs text-muted-foreground tabular-nums">
            {subtitle}
          </span>
        )}
      </span>
      {queueUri && <QueueAction uri={queueUri} disabled={disabled} />}
      {durationMs !== undefined && (
        <span className="w-10 shrink-0 text-right text-xs text-muted-foreground tabular-nums">
          {fmtClockSecs(durationMs / 1000)}
        </span>
      )}
    </>
  );
  const row = "group/row flex w-full min-w-0 items-center gap-3 rounded-lg px-2 py-1.5 text-left";
  if (!onPlay) return <div className={row}>{body}</div>;
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onPlay}
      className={cn(
        row,
        "cursor-pointer transition-[background-color] hover:bg-accent disabled:pointer-events-none disabled:opacity-50",
      )}
    >
      {body}
    </button>
  );
}
