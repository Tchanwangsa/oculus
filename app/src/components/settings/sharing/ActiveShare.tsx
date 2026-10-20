import { useState } from "react";
import { Button } from "@/components/ui/button";
import { CollapsibleSection } from "@/components/ui/CollapsibleSection";
import type { ShareEndpoint, ShareSummary } from "@/lib/sharing/types";
import { devices } from "@/lib/sharing/view";

/** One live share: its canvas, how many devices are connected, the addresses
 * it answers on (folded; no link, since a link holds the secret), and Stop. */
export function ActiveShare({ share, title, endpoints, stopping, onStop }: {
  share: ShareSummary;
  title: string;
  endpoints: readonly ShareEndpoint[];
  stopping: boolean;
  onStop: () => void;
}) {
  const [open, setOpen] = useState(false);
  const state = share.phase === "paused" ? "Paused · the file changed outside Oculus" : share.clients ? `${devices(share.clients)} connected` : "No device connected";
  return (
    <div className="border-b border-border-subtle py-2.5 last:border-b-0">
      <div className="settings-row flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="truncate text-xs text-foreground">{title}</p>
          <p className="mt-0.5 text-[11px] text-muted-foreground tabular-nums">{state}</p>
        </div>
        <Button size="sm" variant="outline" disabled={stopping} onClick={onStop}>{stopping ? "Stopping…" : "Stop"}</Button>
      </div>
      <CollapsibleSection title="Addresses" open={open} onOpenChange={setOpen} count={endpoints.length} className="mt-1 border-b-0">
        {endpoints.length ? (
          <ul className="flex flex-col gap-1 pl-4">
            {endpoints.map((e) => (
              <li key={e.id} className="flex min-w-0 justify-between gap-4 text-[11px]">
                <span className="truncate text-foreground">{e.label}</span>
                <span className="min-w-0 truncate text-muted-foreground tabular-nums">{e.address}</span>
              </li>
            ))}
          </ul>
        ) : <p className="pl-4 text-[11px] text-muted-foreground">This Mac has no address another device could use.</p>}
      </CollapsibleSection>
    </div>
  );
}
