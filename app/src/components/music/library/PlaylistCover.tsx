import { cn } from "@/lib/utils";
import { MusicCover } from "../MusicCover";

/**
 * A cover from a list of images: four as a 2×2 grid, the way Spotify draws
 * a playlist without a picture (the square is rounded, the tiles are not,
 * no gaps), one as a plain cover, none as the note glyph.
 */
export function PlaylistCover({
  images,
  className,
  iconSize,
}: {
  images: string[];
  /** Size and radius, as for `MusicCover`. */
  className?: string;
  iconSize?: number;
}) {
  if (images.length < 4) {
    return <MusicCover url={images[0]} className={className} iconSize={iconSize} />;
  }
  return (
    <div
      aria-hidden
      className={cn("grid shrink-0 grid-cols-2 grid-rows-2 overflow-hidden bg-muted", className)}
    >
      {images.slice(0, 4).map((url, i) => (
        <img
          key={i}
          src={url}
          alt=""
          draggable={false}
          className="size-full min-h-0 min-w-0 object-cover"
        />
      ))}
    </div>
  );
}
