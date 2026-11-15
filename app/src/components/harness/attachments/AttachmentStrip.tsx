import { lazy, Suspense, useState } from "react";

import { PastedTextCard, RemoveAttachment } from "@/components/harness/attachments/PastedText";
import { Button } from "@/components/ui/button";
import { ImageLightbox } from "@/components/ui/lightbox/Lightbox";
import type { ShelfItem } from "@/hooks/agents/useAttachments";
import { type PastedText } from "@/lib/harness/attachments";
import { cn } from "@/lib/utils";

/** On demand: it brings the note editor, which a chat page otherwise never loads. */
const PastedTextEditor = lazy(() =>
  import("@/components/harness/attachments/PastedTextEditor").then((m) => ({ default: m.PastedTextEditor })),
);

/**
 * Pending pasted text cards and the draft's shelf of pictures above a
 * composer; renders nothing when empty. `object-cover` (unlike the thread's
 * `object-contain`) because a thumbnail this small is an identifier; click
 * opens the full image. A text card opens `PastedTextEditor`. Removing a
 * picture the sentence still names asks first, in a line under the strip
 * (`confirming`), then takes its chips too. `compact` is the dock's.
 */
export function AttachmentStrip({
  items,
  onDetach,
  confirming = null,
  onConfirmDetach,
  onCancelDetach,
  texts = [],
  onEditText,
  onDetachText,
  onPutBack,
  subjectId = null,
  compact,
  className,
}: {
  items: ShelfItem[];
  onDetach: (id: string) => void;
  /** The picture waiting on "remove it from the message too". */
  confirming?: string | null;
  onConfirmDetach?: () => void;
  onCancelDetach?: () => void;
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
  const asking = items.find((a) => a.id === confirming);

  return (
    <div className={cn("flex flex-col gap-1.5", className)}>
      <div className="flex flex-wrap items-center gap-2">
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
      {asking && (
        <div role="alert" className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted-foreground">
          <span className="min-w-0">
            “<span className="text-foreground">{asking.name}</span>” is in your message. Remove it there too?
          </span>
          <div className="flex items-center gap-1">
            <Button size="xs" variant="destructive" onClick={onConfirmDetach}>
              Remove
            </Button>
            <Button size="xs" variant="ghost" onClick={onCancelDetach}>
              Keep
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
