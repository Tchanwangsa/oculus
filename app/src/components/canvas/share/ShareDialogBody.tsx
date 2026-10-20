import { Button } from "@/components/ui/button";
import type { CanvasAuthorityPhase } from "@/lib/canvas/authority";
import type { ShareView } from "@/lib/sharing/controller";
import { pickEndpoint, shareStatusLine } from "@/lib/sharing/view";
import { AddressList } from "./AddressList";
import { ShareLink } from "./ShareLink";
import { ShareQr } from "./ShareQr";

export interface ShareDialogActions {
  onStart(): void;
  onStop(): void;
  onSelect(endpointId: string): void;
  onRetry(): void;
  onDismissError(): void;
  onConfirmStop(confirming: boolean): void;
}

const TONE = { muted: "bg-muted-foreground/50", live: "bg-success", warning: "bg-warning" } as const;

/** What the share dialog shows for a controller view; state lives in the
 * caller, so this renders the same on the server in tests. */
export function ShareDialogBody({ view, authority, confirmingStop, ...actions }: ShareDialogActions & {
  view: ShareView;
  authority: CanvasAuthorityPhase;
  confirmingStop: boolean;
}) {
  const line = shareStatusLine(view, authority);
  const share = view.status === "sharing" ? view.share : null;
  const endpoint = share ? pickEndpoint(share.endpoints, view.preferred) : null;
  const noAddress = view.addresses != null && !view.addresses.endpoints.length;
  return (
    <div className="flex min-w-0 flex-col gap-4">
      <p role="status" className="flex items-center gap-2 text-xs text-foreground">
        <span aria-hidden className={`size-2 shrink-0 rounded-full ${TONE[line.tone]}`} />
        {line.text}
      </p>

      {view.error && (
        <div role="alert" className="flex items-start justify-between gap-3 text-xs text-destructive">
          <p className="min-w-0 break-words">{view.error.message}</p>
          <Button size="xs" variant="ghost" onClick={actions.onDismissError}>Dismiss</Button>
        </div>
      )}
      {view.refreshError && (
        <div role="alert" className="flex items-start justify-between gap-3 text-xs text-destructive">
          <p className="min-w-0 break-words">Could not read this Mac’s addresses: {view.refreshError}</p>
          <Button size="xs" variant="outline" onClick={actions.onRetry}>Retry</Button>
        </div>
      )}

      {share ? (
        <>
          {authority === "paused" || share.phase === "paused" ? (
            <p className="text-xs text-muted-foreground">The file changed outside Oculus. Devices can look but not save until you choose a version in the canvas.</p>
          ) : null}
          <div className="flex min-w-0 flex-wrap items-start gap-4">
            {endpoint?.url ? <ShareQr link={endpoint.url} /> : null}
            <div className="min-w-[12rem] flex-1">
              <AddressList endpoints={share.endpoints} selected={endpoint?.id ?? null} onSelect={actions.onSelect} />
            </div>
          </div>
          {endpoint?.url ? <ShareLink link={endpoint.url} /> : null}
          <ShareLimits />
          <StopShare confirming={confirmingStop} stopping={false} onConfirm={actions.onConfirmStop} onStop={actions.onStop} />
        </>
      ) : view.status === "stopping" ? (
        <StopShare confirming={false} stopping onConfirm={actions.onConfirmStop} onStop={actions.onStop} />
      ) : (
        <>
          <p className="text-xs leading-relaxed text-muted-foreground">
            {view.status === "ended"
              ? "Devices can no longer open the old link. Sharing again makes a new link and code."
              : "Open this canvas in a browser on an iPad or phone and draw on it together. You get a link and a QR code."}
          </p>
          {noAddress && <p className="text-xs text-muted-foreground">This Mac has no address another device could use. Join a Wi‑Fi network or a VPN first.</p>}
          <ShareLimits />
          <div>
            <Button size="sm" disabled={view.status === "starting" || view.status === "unknown"} onClick={actions.onStart}>
              {view.status === "starting" ? "Starting…" : view.status === "ended" ? "Share again" : "Start sharing"}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

/** What sharing can and cannot do, said once here and in Settings. */
export function ShareLimits() {
  return (
    <ul className="flex list-disc flex-col gap-1 pl-4 text-[11px] leading-relaxed text-muted-foreground">
      <li>Works on the same network or over your VPN. Public and guest Wi‑Fi often keep devices apart, so a device there may not open the link.</li>
      <li>Anyone with the link can edit this canvas until you stop sharing.</li>
      <li>If this Mac’s address changes, the link and code here update; a device that only has the old link has to open the new one.</li>
    </ul>
  );
}

function StopShare({ confirming, stopping, onConfirm, onStop }: {
  confirming: boolean;
  stopping: boolean;
  onConfirm(confirming: boolean): void;
  onStop(): void;
}) {
  if (stopping) return <div><Button size="sm" variant="outline" disabled>Stopping…</Button></div>;
  if (!confirming) return <div><Button size="sm" variant="outline" onClick={() => onConfirm(true)}>Stop sharing</Button></div>;
  return (
    <div className="flex flex-wrap items-center gap-2">
      <p className="mr-auto text-xs text-foreground">Stop sharing? Devices on this link lose access at once.</p>
      <Button size="sm" variant="destructive" onClick={onStop}>Stop sharing</Button>
      <Button size="sm" variant="ghost" onClick={() => onConfirm(false)}>Cancel</Button>
    </div>
  );
}
