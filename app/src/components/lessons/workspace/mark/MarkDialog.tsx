import { useRef } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { fmtDue } from "@/lib/format/format";
import { markDialogCopy, type MarkPrompt } from "@/lib/ed/markPrompts";

/** The one question before a Mark that needs asking: late, the last attempt,
 *  or a challenge that asks for confirmation (`markDialogCopy`). */
export function MarkDialog({
  prompts,
  dueAt,
  onConfirm,
  onCancel,
}: {
  /** Null while no question is open. */
  prompts: MarkPrompt[] | null;
  dueAt: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  // The last question stays worded while the dialog animates shut.
  const last = useRef({ prompts: prompts ?? [], dueAt });
  if (prompts) last.current = { prompts, dueAt };
  const shown = last.current;
  const due = shown.dueAt && Number.isFinite(Date.parse(shown.dueAt)) ? fmtDue(new Date(shown.dueAt)) : null;
  const copy = markDialogCopy(shown.prompts, due);
  return (
    <Dialog open={prompts != null} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{copy.title}</DialogTitle>
          <DialogDescription asChild>
            <div className="space-y-1.5">
              {copy.lines.map((line) => (
                <p key={line}>{line}</p>
              ))}
            </div>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          <Button onClick={onConfirm}>{copy.confirmLabel}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
