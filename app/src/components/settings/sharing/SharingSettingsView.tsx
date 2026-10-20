import { Section } from "@/components/settings/shared/section";
import { Button } from "@/components/ui/button";
import type { ShareEndpoint, ShareSummary } from "@/lib/sharing/types";
import { ActiveShare } from "./ActiveShare";

export interface SharingSettingsProps {
  shares: readonly { share: ShareSummary; title: string }[];
  endpoints: readonly ShareEndpoint[];
  stopping: ReadonlySet<number>;
  stoppingAll: boolean;
  confirmingAll: boolean;
  error: string | null;
  onStop(fileId: number): void;
  onStopAll(): void;
  onConfirmAll(confirming: boolean): void;
}

/** Settings → Device sharing, from state its caller reads. */
export function SharingSettingsView(props: SharingSettingsProps) {
  const { shares, endpoints, stopping, stoppingAll, confirmingAll, error } = props;
  return (
    <div className="flex flex-col gap-8">
      <Section title="Device sharing" description="Draw on a canvas from an iPad or phone on your network.">
        <ul className="flex list-disc flex-col gap-1.5 pl-4 text-xs leading-relaxed text-muted-foreground">
          <li>Share a canvas from its header. A browser on the same network, or on your VPN, opens it from the link or QR code and edits it with you.</li>
          <li>Oculus answers only on this Mac’s network addresses, without a relay, encryption or a name on the network. Public and guest Wi‑Fi often keep devices apart.</li>
          <li>A link holds the share’s key: anyone with it can edit that canvas until you stop sharing. Stopping makes it useless.</li>
          <li>Sharing is off whenever Oculus starts, and stops when Oculus quits.</li>
        </ul>
      </Section>

      <Section title="Active shares">
        {error && <p role="alert" className="pb-2 text-xs text-destructive">{error}</p>}
        {shares.length === 0 ? (
          <p className="py-2 text-xs text-muted-foreground">Nothing is shared. Open a canvas and choose Share.</p>
        ) : (
          <>
            <div>
              {shares.map(({ share, title }) => (
                <ActiveShare
                  key={share.shareId} share={share} title={title} endpoints={endpoints}
                  stopping={stoppingAll || stopping.has(share.fileId)} onStop={() => props.onStop(share.fileId)}
                />
              ))}
            </div>
            <div className="settings-actions mt-3 flex flex-wrap items-center gap-2">
              {confirmingAll ? (
                <>
                  <p className="mr-auto text-xs text-foreground">Stop every share? Every device loses access at once.</p>
                  <Button size="sm" variant="destructive" onClick={props.onStopAll}>Stop all</Button>
                  <Button size="sm" variant="ghost" onClick={() => props.onConfirmAll(false)}>Cancel</Button>
                </>
              ) : (
                <Button size="sm" variant="outline" disabled={stoppingAll} onClick={() => props.onConfirmAll(true)}>
                  {stoppingAll ? "Stopping…" : "Stop all"}
                </Button>
              )}
            </div>
          </>
        )}
      </Section>
    </div>
  );
}
