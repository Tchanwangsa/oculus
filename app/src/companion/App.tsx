import type { ReactNode } from "react";
import { SpinnerGapIcon } from "@phosphor-icons/react";
import { CanvasSurface } from "@/components/canvas/CanvasSurface";
import { Button } from "@/components/ui/button";
import { TooltipProvider } from "@/components/ui/tooltip";
import { downloadBlob } from "@/lib/canvas/platform";
import type { ShareCredentials } from "@/lib/companion/link";
import type { ShareClient, ShareClientState } from "@/lib/companion/shareClient";
import { ShareHeader } from "./ShareHeader";
import { useShareClient } from "./useShareClient";

/** The camera stays in the surface: pan and zoom never reach the shared file. */
const keepCameraLocal = () => {};

const saveDraft = (text: string | null, name: string) => {
  if (text) downloadBlob(new Blob([text], { type: "application/json" }), name);
};

function Notice({ tone = "muted", children }: { tone?: "muted" | "alert"; children: ReactNode }) {
  return <div role={tone === "alert" ? "alert" : "status"} data-selectable="true"
    className={tone === "alert" ? "flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border border-destructive/20 bg-card px-3 py-2 text-xs text-destructive" : "flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border border-border-subtle bg-card px-3 py-2 text-xs text-muted-foreground"}>
    {children}
  </div>;
}

function Notices({ client, state }: { client: ShareClient; state: ShareClientState }) {
  const download = () => saveDraft(client.draftText(), "Shared canvas draft.canvas");
  return <>
    {state.status === "conflict" ? <Notice tone="alert">
      <span className="mr-auto">{state.lastError} Your drawing here is kept until you choose.</span>
      <Button variant="outline" size="sm" onClick={download}>Download draft</Button>
      <Button size="sm" onClick={() => client.useSharedCanvas()}>Use shared canvas</Button>
    </Notice> : state.lastError && state.status !== "connected" && state.status !== "saving" ? <Notice tone={state.status === "ended" ? "alert" : "muted"}>{state.lastError}</Notice> : null}
    {state.storageWarning ? <Notice>{state.storageWarning}</Notice> : null}
    {state.recovery ? <Notice>
      <span className="mr-auto">A drawing from an earlier sharing session is kept on this device.</span>
      <Button variant="outline" size="sm" onClick={() => saveDraft(client.recoveryText(), "Earlier canvas draft.canvas")}>Download</Button>
      <Button variant="ghost" size="sm" onClick={() => client.dismissRecovery()}>Dismiss</Button>
    </Notice> : null}
  </>;
}

function SharedCanvas({ credentials }: { credentials: ShareCredentials }) {
  const { client, state } = useShareClient(credentials);
  if (!client || !state) return null;
  const blocked = state.status === "conflict" || state.status === "ended";
  return <div className="companion-shell flex h-dvh flex-col gap-2 pt-[max(10px,env(safe-area-inset-top))] pr-[max(12px,env(safe-area-inset-right))] pb-[max(10px,env(safe-area-inset-bottom))] pl-[max(12px,env(safe-area-inset-left))]">
    <ShareHeader state={state} onRetry={() => client.retry()} onDownloadDraft={() => saveDraft(client.draftText(), "Shared canvas draft.canvas")} />
    <main className="relative min-h-0 flex-1 overflow-hidden rounded-2xl border border-border-subtle bg-card">
      <div className="pointer-events-none absolute inset-x-3 bottom-16 z-30 flex max-h-[45%] flex-col gap-2 overflow-y-auto [&>*]:pointer-events-auto" data-canvas-notices>
        <Notices client={client} state={state} />
      </div>
      {state.document
        ? <div inert={blocked} className="h-full"><CanvasSurface document={state.document} onChange={(document) => client.edit(document)} onViewportChange={keepCameraLocal} /></div>
        : <div className="flex h-full items-center justify-center gap-2 text-sm text-muted-foreground">
          {state.status === "ended" ? "This canvas is no longer shared." : <><SpinnerGapIcon className="animate-spin" aria-hidden />Opening the canvas from the Mac…</>}
        </div>}
    </main>
  </div>;
}

/** A page opened without a complete link has nothing to connect to. */
function MissingLink() {
  return <div className="flex h-dvh items-center justify-center p-6">
    <div className="max-w-sm text-center">
      <h1 className="text-[18px] font-semibold tracking-tight">Open the full share link</h1>
      <p className="mt-2 text-[13px] leading-relaxed text-muted-foreground">This page needs the link from Share on the Mac, including the part after #. Scan the QR code again or copy the whole link.</p>
    </div>
  </div>;
}

export function App({ credentials }: { credentials: ShareCredentials | null }) {
  return <TooltipProvider>{credentials ? <SharedCanvas credentials={credentials} /> : <MissingLink />}</TooltipProvider>;
}
