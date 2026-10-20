/** What the share dialog and Settings say about a share, derived from the
 * controller's view; no claim goes past what the native side reported. */
import type { CanvasAuthorityPhase } from "@/lib/canvas/authority";
import type { ShareView } from "./controller";
import type { ShareEndpoint } from "./types";

/** Keep an explicit choice; otherwise prefer an address a device is already using. */
export function pickEndpoint(endpoints: readonly ShareEndpoint[], preferred: string | null): ShareEndpoint | null {
  const linked = endpoints.filter((e) => e.url);
  return linked.find((e) => e.id === preferred)
    ?? linked.find((e) => e.connectedClients > 0 && e.family === "ipv4")
    ?? linked.find((e) => e.connectedClients > 0)
    ?? linked.find((e) => e.family === "ipv4")
    ?? linked[0] ?? null;
}

export const devices = (n: number) => (n === 1 ? "1 device" : `${n} devices`);

export interface ShareStatusLine {
  text: string;
  tone: "muted" | "live" | "warning";
}

export function shareStatusLine(view: ShareView, authority: CanvasAuthorityPhase): ShareStatusLine {
  switch (view.status) {
    case "unknown": return { text: "Checking…", tone: "muted" };
    case "starting": return { text: "Starting…", tone: "muted" };
    case "stopping": return { text: "Stopping…", tone: "muted" };
    case "ended": return { text: "Sharing ended", tone: "warning" };
    case "off": return { text: "Not shared", tone: "muted" };
    case "sharing": {
      if (view.share?.phase === "paused" || authority === "paused") return { text: "Paused · the file changed outside Oculus", tone: "warning" };
      const clients = view.share?.clients ?? 0;
      return { text: clients ? `Sharing · ${devices(clients)} connected` : "Sharing · no device connected", tone: "live" };
    }
  }
}
