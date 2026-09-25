import { useState } from "react";
import { X } from "@phosphor-icons/react";

import { ImageLightbox } from "@/components/ui/Lightbox";
import { type PendingAttachment } from "@/lib/attachments";
import { cn } from "@/lib/utils";

/**
 * Pending picture thumbnails above a composer; renders nothing when empty.
 * `object-cover` (unlike the thread's `object-contain`) because a thumbnail this
 * small is an identifier; click opens the full image. `compact` is the dock's.
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
