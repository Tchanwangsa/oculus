import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useLeaveLecture } from "@/stores/leaveLectureStore";

/**
 * Asks before a playing lecture is left with no tab to come back to. Mounted
 * once, in the shell — the things that raise it (the tab strip's ×, a
 * navigation out of the lecture's own tab) don't share a subtree.
 *
 * It says where the progress went, because that is the actual worry behind the
 * hesitation, and the answer is that it is already saved.
 */
export function LeaveLectureDialog() {
  const pending = useLeaveLecture((s) => s.pending);
  const confirm = useLeaveLecture((s) => s.confirm);
  const cancel = useLeaveLecture((s) => s.cancel);

  return (
    <Dialog open={!!pending} onOpenChange={(open) => !open && cancel()}>
      <DialogContent className="sm:max-w-sm" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Leave this lecture?</DialogTitle>
          <DialogDescription>
            {pending?.title
              ? `“${pending.title}” is still playing. Your place is saved — you can pick it up where you left off.`
              : "The lecture is still playing. Your place is saved."}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={cancel}>
            Keep watching
          </Button>
          <Button onClick={confirm}>Leave</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
