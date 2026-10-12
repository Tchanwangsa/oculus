import { useState } from "react";
import { VideoCamera } from "@phosphor-icons/react";

/**
 * A card's 16:9 tile: the picture at `src`, or a camera glyph without one or
 * once it fails to load. `src` stays empty until it is known, since an empty
 * `src` attribute fires `error`. Key it by `src` so a new picture retries.
 */
export function Thumbnail({ src }: { src?: string | null }) {
  const [broken, setBroken] = useState(false);
  const shown = src && !broken ? src : "";
  return (
    <span
      className="flex aspect-video w-32 shrink-0 items-center justify-center overflow-hidden rounded-md border border-border-subtle bg-surface-raised text-muted-foreground"
    >
      {shown ? (
        <img
          src={shown}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          onError={() => setBroken(true)}
          className="block size-full object-cover"
        />
      ) : (
        <VideoCamera size={18} />
      )}
    </span>
  );
}
