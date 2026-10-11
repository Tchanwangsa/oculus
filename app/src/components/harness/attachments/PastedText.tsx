import { useRef, useState } from "react";
import { Check, Copy, TextAlignLeft, X } from "@phosphor-icons/react";

import { CompactMd } from "@/components/markdown/MdComponents";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { lineCount, pastedBlock } from "@/lib/harness/attachments";
import { cn, copyText } from "@/lib/utils";

/** The hover-revealed remove button on a composer's thumbnail or text card;
 *  its parent carries `group/att`. */
export function RemoveAttachment({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className="absolute -right-1.5 -top-1.5 flex h-[18px] w-[18px] cursor-pointer items-center justify-center rounded-full border border-border bg-card text-muted-foreground opacity-0 transition-opacity will-change-[opacity] group-hover/att:opacity-100 hover:text-foreground focus-visible:opacity-100"
    >
      <X size={9} weight="bold" />
    </button>
  );
}

function plural(n: number, one: string): string {
  return `${n.toLocaleString()} ${n === 1 ? one : `${one}s`}`;
}

/** The card's opening words; CSS truncates, so this only bounds the DOM. */
function snippet(text: string): string {
  return text.trim().replace(/\s+/g, " ").slice(0, 120) || "Empty";
}

/**
 * A pasted text as a card the height of a picture thumbnail: its opening words
 * and line count. The composer's strip and a sent bubble share it. A copied
 * selection across it takes its `<pasted_text>` block (`data-md`). Opened, a
 * held card edits in `PastedTextEditor` (its own file, loaded on demand: it
 * brings the note editor) and a sent one reads in `PastedTextViewer`.
 */
export function PastedTextCard({
  text,
  compact,
  onOpen,
  onRemove,
}: {
  text: string;
  /** The dock's size. */
  compact?: boolean;
  onOpen: () => void;
  onRemove?: () => void;
}) {
  return (
    <div data-md={pastedBlock(text)} className="group/att relative">
      <button
        type="button"
        onClick={onOpen}
        aria-label="Open pasted text"
        className={cn(
          "flex cursor-pointer items-center gap-2 overflow-hidden rounded-lg border border-border bg-surface text-left transition-colors hover:border-ring",
          compact ? "h-10 w-36 px-2" : "h-14 w-48 px-2.5",
        )}
      >
        <TextAlignLeft size={compact ? 14 : 16} className="shrink-0 text-muted-foreground" />
        <span className="flex min-w-0 flex-col">
          <span className={cn("truncate text-foreground", compact ? "text-[11px]" : "text-[12px]")}>
            {snippet(text)}
          </span>
          <span className="text-[11px] text-muted-foreground tabular-nums">
            {plural(lineCount(text), "line")}
          </span>
        </span>
      </button>
      {onRemove && <RemoveAttachment label="Remove pasted text" onClick={onRemove} />}
    </div>
  );
}

/** The pasted text dialogs' quiet secondary line. */
export function PastedTextCounts({ text }: { text: string }) {
  return (
    <DialogDescription className="tabular-nums">
      {plural(lineCount(text), "line")} · {plural(text.length, "character")}
    </DialogDescription>
  );
}

/** A sent card opened read-only: the text as rendered markdown, to copy. */
export function PastedTextViewer({ text, onClose }: { text: string; onClose: () => void }) {
  const [copied, setCopied] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent showCloseButton={false} className="sm:max-w-[760px]">
        <DialogHeader>
          <DialogTitle>Pasted text</DialogTitle>
          <PastedTextCounts text={text} />
        </DialogHeader>
        <div ref={scrollRef} className="max-h-[65vh] overflow-x-hidden overflow-y-auto">
          <CompactMd text={text} className="text-[13px] leading-relaxed" />
        </div>
        <DialogFooter>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              void copyText(text).then((ok) => {
                if (!ok) return;
                setCopied(true);
                setTimeout(() => setCopied(false), 1200);
              });
            }}
          >
            {copied ? <Check /> : <Copy />}
            {copied ? "Copied" : "Copy"}
          </Button>
          <Button size="sm" onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
