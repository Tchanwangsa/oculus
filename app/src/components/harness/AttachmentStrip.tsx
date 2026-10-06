import { lazy, Suspense, useState } from "react";

import { PastedTextCard, RemoveAttachment } from "@/components/harness/PastedText";
import { ImageLightbox } from "@/components/ui/Lightbox";
import { type PastedText, type PendingAttachment } from "@/lib/attachments";
import { cn } from "@/lib/utils";

/** On demand: it brings the note editor, which a chat page otherwise never loads. */
const PastedTextEditor = lazy(() =>
  import("@/components/harness/PastedTextEditor").then((m) => ({ default: m.PastedTextEditor })),
);

/**
 * Pending pasted text cards and picture thumbnails above a composer, in the
 * order the message will carry them; renders nothing when empty. `object-cover`
 * (unlike the thread's `object-contain`) because a thumbnail this small is an
 * identifier; click opens the full image. A text card opens
 * `PastedTextEditor`. `compact` is the dock's.
 */
export function AttachmentStrip({
  items,
  onDetach,
  texts = [],
  onEditText,
  onDetachText,
  onPutBack,
  subjectId = null,
  compact,
  className,
}: {
  items: PendingAttachment[];
  onDetach: (id: string) => void;
  texts?: PastedText[];
  onEditText?: (id: string, text: string) => void;
  onDetachText?: (id: string) => void;
  /** Puts a card's text into the message box; the strip then drops the card. */
  onPutBack?: (text: string) => void;
  /** The `@` scope inside an opened text card. */
  subjectId?: number | null;
  compact?: boolean;
  className?: string;
}) {
  const [shown, setShown] = useState<string | null>(null);
  const [openText, setOpenText] = useState<string | null>(null);
  if (!items.length && !texts.length) return null;
  const opened = texts.find((t) => t.id === openText);

  return (
    <div className={cn("flex flex-wrap items-center gap-2", className)}>
      <ImageLightbox
        src={shown ?? ""}
        alt={items.find((a) => a.preview === shown)?.name}
        open={shown !== null}
        onOpenChange={(o) => !o && setShown(null)}
      />
      {opened && (
        <Suspense fallback={null}>
          <PastedTextEditor
            key={opened.id}
            text={opened.text}
            subjectId={subjectId}
            onDone={(text) => {
              if (text !== opened.text) onEditText?.(opened.id, text);
              setOpenText(null);
            }}
            onRemove={() => {
              onDetachText?.(opened.id);
              setOpenText(null);
            }}
            onPutBack={(text) => {
              onPutBack?.(text);
              onDetachText?.(opened.id);
              setOpenText(null);
            }}
          />
        </Suspense>
      )}
      {texts.map((t) => (
        <PastedTextCard
          key={t.id}
          text={t.text}
          compact={compact}
          onOpen={() => setOpenText(t.id)}
          onRemove={() => onDetachText?.(t.id)}
        />
      ))}
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
          <RemoveAttachment label={`Remove ${a.name}`} onClick={() => onDetach(a.id)} />
        </div>
      ))}
    </div>
  );
}
