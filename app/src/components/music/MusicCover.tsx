import { MusicNotes } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/** A track's album cover, or a note glyph on a muted square without one. */
export function MusicCover({
  url,
  className,
  iconSize = 18,
}: {
  url: string | null | undefined;
  /** Size and radius. */
  className?: string;
  iconSize?: number;
}) {
  if (url) {
    return (
      <img
        src={url}
        alt=""
        draggable={false}
        className={cn("shrink-0 bg-muted object-cover", className)}
      />
    );
  }
  return (
    <div
      aria-hidden
      className={cn(
        "flex shrink-0 items-center justify-center bg-muted text-muted-foreground",
        className,
      )}
    >
      <MusicNotes size={iconSize} />
    </div>
  );
}
