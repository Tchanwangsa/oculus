import { useState } from "react";
import { ImageLightbox } from "@/components/ui/lightbox/Lightbox";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { attachmentSrc } from "@/lib/harness/attachments";
import { basename, sourceOf } from "@/components/markdown/embed/shared";

export function EmbedImage({ path, alt }: { path: string; alt?: string }) {
  const dataDir = useDataDir();
  const src = attachmentSrc(dataDir, path);
  const [broken, setBroken] = useState(false);
  const [open, setOpen] = useState(false);
  const label = alt || basename(path);

  // The alt or filename, never WebKit's broken-image glyph.
  if (broken)
    return (
      <span data-md={sourceOf(path, alt)} title={path} className="text-muted-foreground">
        {label}
      </span>
    );

  return (
    <span data-md={sourceOf(path, alt)} className="my-3 block">
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-label={`Open ${label}`}
        className="block max-w-full cursor-zoom-in"
      >
        {/* Not until the data dir resolves: an empty `src` fires `error`. */}
        {src && (
          <img
            src={src}
            alt={alt ?? ""}
            title={path}
            onError={() => setBroken(true)}
            className="block max-h-80 w-auto max-w-full rounded-lg border border-border"
          />
        )}
      </button>
      {alt && <span className="mt-1.5 block text-xs text-muted-foreground">{alt}</span>}
      <ImageLightbox src={src} alt={alt} open={open} onOpenChange={setOpen} />
    </span>
  );
}
