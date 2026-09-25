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
 * once in the shell, since its triggers (tab ×, navigating away) share no
 * subtree. It says the place is saved, which is the actual worry.
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
