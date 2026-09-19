import { useState } from "react";
import { X } from "@phosphor-icons/react";

import { ImageLightbox } from "@/components/ui/Lightbox";
import { type PendingAttachment } from "@/lib/attachments";
import { cn } from "@/lib/utils";

/**
 * The pictures a composer is holding, as a row of thumbnails above its text.
 *
 * Shared by every box that takes an attachment (`useAttachments`), so a
 * screenshot pasted into the dock is drawn, removed and opened exactly as one
 * pasted into the page composer. It renders nothing at all when the list is
 * empty, which is why every call site can mount it unconditionally.
 *
 * `object-cover` here and `object-contain` in the thread, and that is not an
 * inconsistency: a chip this small is an identifier, where a crop that fills
 * the square tells two screenshots apart better than a letterboxed thumbnail
 * two thirds of which is ground. The full picture is one click away — the same
 * viewer the thread's own cards open (`ImageLightbox`), because 56px of a
 * screenshot is enough to tell two apart and not enough to check one.
 *
 * `compact` is the lecture dock's: a 300px panel beside a playing video has
 * room for a smaller identifier and needs the lines above the composer more
 * than the page does.
 */
export function AttachmentStrip({
  items,
  onDetach,
  compact,
  className,
}: {
  items: PendingAttachment[];
  onDetach: (id: string) => void;
  compact?: boolean;
  className?: string;
}) {
  const [shown, setShown] = useState<string | null>(null);
  if (!items.length) return null;

  return (
    <div className={cn("flex flex-wrap items-center gap-2", className)}>
      <ImageLightbox
        src={shown ?? ""}
        alt={items.find((a) => a.preview === shown)?.name}
        open={shown !== null}
        onOpenChange={(o) => !o && setShown(null)}
      />
      {items.map((a) => (
        <div key={a.id} className="group/att relative">
          <button
            type="button"
            onClick={() => setShown(a.preview)}
            aria-label={`Open ${a.name}`}
            title={a.name}
            className="block cursor-pointer overflow-hidden rounded-lg border border-border transition-colors hover:border-ring"
          >
            <img
              src={a.preview}
              alt={a.name}
              className={cn("block object-cover", compact ? "h-10 w-10" : "h-14 w-14")}
            />
          </button>
          <button
            type="button"
            aria-label={`Remove ${a.name}`}
            onClick={() => onDetach(a.id)}
            className="absolute -right-1.5 -top-1.5 flex h-[18px] w-[18px] cursor-pointer items-center justify-center rounded-full border border-border bg-card text-muted-foreground opacity-0 transition-opacity group-hover/att:opacity-100 hover:text-foreground focus-visible:opacity-100"
          >
            <X size={9} weight="bold" />
          </button>
        </div>
      ))}
    </div>
  );
}
