import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DriveTree } from "../tree/DriveTree";

/**
 * Pick one of the student's folders in a subject, for Copy to my files… and
 * Move to…: the subject's folder tree, where only folders they may add to
 * can be picked.
 */
export function FolderPickerDialog({ request, onCancel, onPick }: {
  request: { title: string; action: string; subjectRel: string; start: string | null } | null;
  onCancel: () => void;
  onPick: (targetDir: string) => void;
}) {
  const [picked, setPicked] = useState<string | null>(null);
  useEffect(() => setPicked(request?.start ?? null), [request]);
  return (
    <Dialog open={!!request} onOpenChange={(o) => !o && onCancel()}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{request?.title}</DialogTitle>
          <DialogDescription>Choose one of your folders.</DialogDescription>
        </DialogHeader>
        <div className="max-h-72 overflow-y-auto rounded-lg border border-border-subtle p-1">
          {request && (
            <DriveTree
              root={request.subjectRel}
              current={picked}
              onPick={setPicked}
              storageKey="oculus-drive-picker-open"
            />
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>Cancel</Button>
          <Button disabled={!picked} onClick={() => picked && onPick(picked)}>{request?.action}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
