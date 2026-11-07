import { createContext, type ReactNode } from "react";
import { useAssistantSend } from "./assistant";
import { fixRequest } from "../widget/fixRequest";

/** Whether the current prefix still has unresolved content. A whole program
 * can wait for its reply to commit while keeping interactions disabled. */
export const StreamingPending = createContext(true);

export function StreamingStatus() {
  return <span role="status" className="text-shimmer inline-flex h-7 items-center text-xs">More coming…</span>;
}

/** Shimmer bars for content still arriving; the frame's footer carries the
 *  words, so a card never says "More coming" twice. Intrinsic CSS sizes keep
 *  placeholders independent of page zoom. */
export function MoreComing() {
  return <div role="status" className="space-y-2 py-3" aria-label="More coming">
    <div aria-hidden="true" className="block-shimmer h-3 w-3/4 rounded" />
    <div aria-hidden="true" className="block-shimmer h-3 w-1/2 rounded" />
  </div>;
}

export function OpenUISkeleton({ title = "Drawing…" }: { title?: string }) {
  return <section aria-busy="true" className="my-3 min-w-0 rounded-xl border border-border bg-card px-4 py-3">
    <span className="text-shimmer text-[13px]">{title}</span>
    <MoreComing />
  </section>;
}

export function OpenUIFailure({ source, code }: { source: string; code: ReactNode }) {
  const send = useAssistantSend();
  return <div className="my-3 flex items-start gap-3 rounded-xl border border-border bg-card px-4 py-3 text-[13px]">
    <details className="min-w-0 flex-1">
      <summary className="flex cursor-pointer list-none flex-wrap items-center gap-x-3 gap-y-1 [&::-webkit-details-marker]:hidden">
        <span role="alert" className="text-muted-foreground">Couldn't draw this</span>
        <span className="text-xs text-muted-foreground hover:text-foreground">Show source</span>
      </summary>
      {code}
    </details>
    {send && <button type="button" className="shrink-0 cursor-pointer text-brand hover:underline"
      onClick={() => send(fixRequest("OpenUI", "Couldn't draw this", source, "openui"))}>Ask to fix</button>}
  </div>;
}
