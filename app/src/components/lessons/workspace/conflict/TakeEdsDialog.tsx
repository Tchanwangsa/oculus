import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { isMac } from "@/lib/platform";

/** Take Ed's replaces the library copy, so it asks first. */
export function TakeEdsDialog({
  open,
  path,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  path: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onCancel()}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Take Ed's version?</DialogTitle>
          <DialogDescription>
            Your copy of {path} on this {isMac ? "Mac" : "PC"} is replaced with the version on Ed, and your
            changes to it are lost.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={onConfirm}>
            Take Ed's
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
