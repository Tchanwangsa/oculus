import { cn } from "@/lib/utils";
import { MusicCover } from "../MusicCover";

/** One playlist, album or track in the player's lists: a click plays it. */
export function LibraryRow({
  name,
  subtitle,
  imageUrl,
  current = false,
  disabled = false,
  onPlay,
}: {
  name: string;
  subtitle: string;
  imageUrl: string | null;
  /** The track playing now. */
  current?: boolean;
  disabled?: boolean;
  onPlay: () => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onPlay}
      className="flex w-full min-w-0 cursor-pointer items-center gap-3 rounded-lg px-2 py-1.5 text-left transition-[background-color] hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
    >
      <MusicCover url={imageUrl} className="size-9 rounded-md" iconSize={15} />
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
    </button>
  );
}
