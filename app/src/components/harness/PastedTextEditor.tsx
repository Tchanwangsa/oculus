import { useCallback, useRef, useState } from "react";

import { NoteField } from "@/components/documents/NoteField";
import { PastedTextCounts } from "@/components/harness/PastedText";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { useDataDir } from "@/hooks/useDataDir";
import { attachmentPath, attachmentSrc } from "@/lib/attachments";

/** Pasted text goes into the message as words; a picture has its own way in. */
const NOT_HERE = "Pictures go in the message box, not in pasted text.";
const refusePicture = () => Promise.reject(NOT_HERE);
const noCommit = () => {};

/**
 * A held card opened as a markdown document on `NoteField`. The text is read
 * through `onChange` as it is edited, so every button sees the latest words
 * without waiting on the field's blur. Escape and the overlay are Done.
 */
export function PastedTextEditor({
  text: initial,
  subjectId,
  onDone,
  onRemove,
  onPutBack,
}: {
  text: string;
  /** `@` searches this subject, or the whole library when null. */
  subjectId: number | null;
  /** Keeps the edited text and closes. */
  onDone: (text: string) => void;
  onRemove: () => void;
  /** Moves the edited text into the message box and drops the card. */
  onPutBack: (text: string) => void;
}) {
  const [text, setText] = useState(initial);
  const latest = useRef(initial);
  const contentRef = useRef<HTMLDivElement>(null);
  const dataDir = useDataDir();

  const imageSrc = useCallback(
    (src: string) => {
      const picture = attachmentPath(src);
      return picture ? attachmentSrc(dataDir, picture) : src;
    },
    [dataDir],
  );
  const onChange = useCallback((next: string) => {
    latest.current = next;
    setText(next);
  }, []);

  return (
    <Dialog open onOpenChange={(open) => !open && onDone(latest.current)}>
      <DialogContent
        ref={contentRef}
        showCloseButton={false}
        className="sm:max-w-[760px]"
        onOpenAutoFocus={(e) => {
          // Straight into the text rather than the first button.
          e.preventDefault();
          contentRef.current?.querySelector<HTMLElement>(".cm-content")?.focus();
        }}
      >
        <DialogHeader>
          <DialogTitle>Pasted text</DialogTitle>
          <PastedTextCounts text={text} />
        </DialogHeader>
        <NoteField
          className="-mx-2"
          scrollClassName="max-h-[65vh]"
          text={initial}
          subjectId={subjectId}
          imageSrc={imageSrc}
          writePicture={refusePicture}
          pickerTitle="Add pictures"
          notAPicture={NOT_HERE}
          placeholder="Pasted text"
          label="Pasted text"
          onCommit={noCommit}
          onChange={onChange}
        />
        <DialogFooter className="sm:justify-between">
          <Button variant="ghost" size="sm" onClick={onRemove}>
            Remove
          </Button>
          <div className="flex flex-col-reverse gap-2 sm:flex-row">
            <Button variant="outline" size="sm" onClick={() => onPutBack(latest.current)}>
              Put in message
            </Button>
            <Button size="sm" onClick={() => onDone(latest.current)}>
              Done
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
