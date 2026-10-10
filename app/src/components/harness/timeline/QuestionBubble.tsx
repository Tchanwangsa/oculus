import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { ArrowCounterClockwise, CaretDown, PencilSimple, X } from "@phosphor-icons/react";
import { FileChip } from "@/components/markdown/FileChip";
import { openCitation, splitLibraryPaths } from "@/lib/files/openFile";
import { attachmentSrc, splitPastedText } from "@/lib/harness/attachments";
import { ImageLightbox } from "@/components/ui/lightbox/Lightbox";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import { PastedTextCard, PastedTextViewer } from "../attachments/PastedText";
import { liftPictures } from "./liftPictures";
import { Action, CopyAction, MessageActions } from "./MessageActions";
import { QUESTION_MAX_H, useOverflows } from "./useOverflows";

/** The edit box grows to its content up to this. */
const EDIT_MAX_H = 240;

/** A question, on the right. Editing (a sent or a queued one) happens in the
 *  bubble itself, grown to the column's width. */
export function QuestionBubble({
  msgId,
  text,
  pending,
  when,
  at,
  onSubmit,
  onRemove,
  onRewind,
}: {
  /** The row id, for the rail to measure. Absent on a queued message. */
  msgId?: number;
  text: string;
  pending?: boolean;
  when?: string;
  /** The playhead second, for a dock message. */
  at?: number | null;
  /** Absent while the thread is busy: a rewind would delete rows mid-write. */
  onSubmit?: (text: string) => void;
  onRemove?: () => void;
  onRewind?: () => void;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  /** The picture open in the lightbox, as the src the card drew. */
  const [shown, setShown] = useState<string | null>(null);
  /** The pasted text open in its viewer, by index. */
  const [viewing, setViewing] = useState<number | null>(null);
  const dataDir = useDataDir();
  // Pasted text draws as cards beside the pictures, not in the words.
  const { text: words, pasted } = useMemo(() => splitPastedText(text), [text]);
  const [body, long] = useOverflows(words, editing === null);
  const box = useRef<HTMLTextAreaElement>(null);
  // Mentions draw as chips rather than the paths the agent was sent.
  const parts = useMemo(() => splitLibraryPaths(words), [words]);
  // Only the prose is measured and folded; cards and pictures always show.
  const [pictures, prose] = useMemo(() => liftPictures(parts), [parts]);

  useLayoutEffect(() => {
    const el = box.current;
    if (!el || editing === null) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, EDIT_MAX_H)}px`;
  }, [editing]);

  if (editing !== null && onSubmit) {
    const save = () => {
      const t = editing.trim();
      setEditing(null);
      if (t && t !== text) onSubmit(t);
    };
    return (
      <div className="w-full rounded-2xl border border-border bg-surface px-4 py-3">
        <Textarea
          ref={box}
          autoFocus
          value={editing}
          onChange={(e) => setEditing(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              setEditing(null);
            }
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              save();
            }
          }}
          rows={1}
          className="max-h-60 w-full resize-none overflow-y-auto border-0 bg-transparent p-0 text-[13px]! leading-relaxed shadow-none focus-visible:border-0 focus-visible:ring-0 dark:bg-transparent"
        />
        <div className="mt-3 flex items-center justify-end gap-2">
          <Button size="xs" variant="outline" onClick={() => setEditing(null)}>
            Cancel
          </Button>
          <Button size="xs" onClick={save}>
            {pending ? "Save" : "Send"}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="group/msg flex w-full flex-col">
      {/* One viewer per message, not per thumbnail. */}
      <ImageLightbox
        src={shown ?? ""}
        alt="Attached picture"
        open={shown !== null}
        onOpenChange={(o) => !o && setShown(null)}
      />
      {viewing !== null && pasted[viewing] != null && (
        <PastedTextViewer text={pasted[viewing]} onClose={() => setViewing(null)} />
      )}
      {/* What the rail measures: attachments and words, not the action row. */}
      <div data-msg-id={msgId} className="flex w-full flex-col items-end gap-1.5">
        {(pasted.length > 0 || pictures.length > 0) && (
          // Pasted text cards first, in the order sent. One picture at its own
          // size; several in a three-across grid, letterboxed rather than
          // cropped — they are usually screenshots.
          <div className="flex max-w-[70%] flex-wrap items-end justify-end gap-1.5">
            {pasted.map((t, i) => (
              <PastedTextCard key={`t${i}`} text={t} onOpen={() => setViewing(i)} />
            ))}
            {pictures.map((p, i) => (
              <button
                key={i}
                type="button"
                aria-label="Open the attached picture"
                onClick={() => setShown(attachmentSrc(dataDir, p.path))}
                className={cn(
                  "cursor-pointer overflow-hidden rounded-xl border border-border bg-surface transition-colors hover:border-ring",
                  pictures.length === 1
                    ? "max-w-full"
                    : "aspect-video w-[calc((100%-0.75rem)/3)]",
                )}
              >
                <img
                  src={attachmentSrc(dataDir, p.path)}
                  alt="Attached picture"
                  className={cn(
                    "block",
                    pictures.length === 1
                      ? "max-h-72 w-auto max-w-full"
                      : "h-full w-full object-contain",
                  )}
                />
              </button>
            ))}
          </div>
        )}
        {(prose.length > 0 || (pictures.length === 0 && pasted.length === 0)) && (
          <div
            className={cn(
              "max-w-[70%] min-w-0 rounded-xl border px-3.5 py-2 text-[13px] leading-relaxed",
              pending
                ? "border-dashed border-border bg-transparent text-muted-foreground"
                : "border-border bg-surface text-foreground",
            )}
          >
            <div
              ref={body}
              data-selectable
              // A mask, not a gradient: a queued bubble's ground is transparent.
              className={cn(
                "whitespace-pre-wrap break-words",
                long && !open && "overflow-hidden [mask-image:linear-gradient(to_bottom,#000_calc(100%-2.25rem),transparent)]",
              )}
              style={long && !open ? { maxHeight: QUESTION_MAX_H } : undefined}
            >
              {prose.map((p, i) => {
                if (p.kind === "text") return p.text;
                if (p.kind !== "path") return null;
                return (
                  <FileChip
                    key={i}
                    path={p.path}
                    cite={p.cite}
                    onClick={(newTab) => openCitation(p.cite, newTab)}
                  />
                );
              })}
            </div>
            {long && (
              <button
                type="button"
                data-copy-skip
                aria-expanded={open}
                onClick={() => setOpen((o) => !o)}
                className="mt-1.5 flex cursor-pointer items-center gap-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
              >
                {open ? "Show less" : "Show more"}
                <CaretDown size={11} className={cn("transition-transform", open && "rotate-180")} />
              </button>
            )}
          </div>
        )}
      </div>
      <MessageActions when={pending ? "Queued" : when} at={pending ? null : at} side="right">
        <CopyAction text={text} />
        {onSubmit && <Action label="Edit" icon={PencilSimple} onClick={() => setEditing(text)} />}
        {onRewind && <Action label="Rewind to here" icon={ArrowCounterClockwise} onClick={onRewind} />}
        {onRemove && <Action label="Remove" icon={X} onClick={onRemove} />}
      </MessageActions>
    </div>
  );
}
