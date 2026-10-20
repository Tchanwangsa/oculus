import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import type { DbFile } from "@/lib/db";
import type { CanvasSession } from "@/lib/canvas/sessions";
import { shareControllerFor } from "@/lib/sharing/registry";
import { ShareDialogBody } from "./share/ShareDialogBody";

/** Share to a device: start, the link and QR per address, stop. Sharing goes on
 * after the dialog closes; the canvas header says so (docs/device-sharing.md). */
export function CanvasShareDialog({ session, open, onOpenChange }: {
  session: CanvasSession<DbFile>;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const controller = useMemo(() => shareControllerFor(session), [session]);
  const view = useSyncExternalStore(controller.subscribe, controller.view);
  const authority = useSyncExternalStore(session.subscribe, () => session.authority);
  const [confirmingStop, setConfirmingStop] = useState(false);

  useEffect(() => {
    if (!open) { setConfirmingStop(false); return; }
    // `companion_endpoints` rereads the addresses first, so opening, focusing the
    // window or showing it again catches a change; reading never starts or stops a share.
    const refresh = () => { if (document.visibilityState === "visible") void controller.refresh(); };
    refresh();
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => { window.removeEventListener("focus", refresh); document.removeEventListener("visibilitychange", refresh); };
  }, [open, controller]);
  useEffect(() => { if (view.status !== "sharing") setConfirmingStop(false); }, [view.status]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-h-[calc(100vh-2rem)] overflow-y-auto sm:max-w-md"
        onEscapeKeyDown={(event) => { if (confirmingStop) { event.preventDefault(); setConfirmingStop(false); } }}
      >
        <DialogHeader>
          <DialogTitle>Share to a device</DialogTitle>
          <DialogDescription>Draw on this canvas from an iPad or phone on your network.</DialogDescription>
        </DialogHeader>
        <ShareDialogBody
          view={view} authority={authority} confirmingStop={confirmingStop}
          onStart={() => void controller.start()}
          onStop={() => { setConfirmingStop(false); void controller.stop(); }}
          onSelect={controller.select}
          onRetry={() => void controller.refresh()}
          onDismissError={controller.dismissError}
          onConfirmStop={setConfirmingStop}
        />
      </DialogContent>
    </Dialog>
  );
}
