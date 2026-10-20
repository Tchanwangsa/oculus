import { ArrowClockwiseIcon, DownloadSimpleIcon, UsersIcon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import type { ShareClientState, ShareStatus } from "@/lib/companion/shareClient";
import { cn } from "@/lib/utils";

const LABEL: Record<ShareStatus, string> = {
  connecting: "Connecting to the Mac…",
  connected: "Saved on the Mac",
  saving: "Saving…",
  reconnecting: "Reconnecting…",
  "offline-unsaved": "Offline · not saved yet",
  conflict: "Draft kept",
  paused: "Saving paused",
  ended: "Sharing ended",
};

const DOT: Record<ShareStatus, string> = {
  connecting: "bg-muted-foreground/50 animate-pulse",
  connected: "bg-success",
  saving: "bg-brand animate-pulse",
  reconnecting: "bg-warning animate-pulse",
  "offline-unsaved": "bg-warning",
  conflict: "bg-destructive",
  paused: "bg-warning",
  ended: "bg-muted-foreground",
};

/** What the page shows, said plainly: status, devices, and the way out with a draft. */
export function ShareHeader({ state, onDownloadDraft, onRetry }: { state: ShareClientState; onDownloadDraft: () => void; onRetry: () => void }) {
  const { status } = state;
  const offerDraft = state.hasDraft && (status === "conflict" || status === "ended" || status === "offline-unsaved" || status === "paused");
  const offerRetry = status === "reconnecting" || status === "offline-unsaved" || status === "paused";
  return <header className="flex h-9 shrink-0 items-center gap-2 px-1">
    <div className="mr-auto flex min-w-0 items-center gap-2">
      <h1 className="font-display text-[15px] font-semibold tracking-tight">Oculus</h1>
      <span className="inline-flex min-w-0 items-center gap-2 rounded-full border border-border-subtle bg-card px-3 py-1 text-xs" role="status" aria-live="polite">
        <span className={cn("size-2 shrink-0 rounded-full", DOT[status])} aria-hidden />
        <span className="truncate">{LABEL[status]}</span>
        {status === "saving" && state.pendingOps > 1 ? <span className="text-muted-foreground tabular-nums">{state.pendingOps} changes</span> : null}
      </span>
      {state.clients > 0 && status !== "ended" ? <span className="hidden shrink-0 items-center gap-1.5 text-xs min-[600px]:inline-flex text-muted-foreground tabular-nums" title="Devices on this canvas">
        <UsersIcon size={14} aria-hidden />{state.clients} {state.clients === 1 ? "device" : "devices"}
      </span> : null}
    </div>
    <div className="flex shrink-0 items-center gap-2">
      {offerRetry ? <Button variant="ghost" size="sm" onClick={onRetry}><ArrowClockwiseIcon />Try now</Button> : null}
      {offerDraft ? <Button variant="outline" size="sm" onClick={onDownloadDraft}><DownloadSimpleIcon />Download draft</Button> : null}
    </div>
  </header>;
}
