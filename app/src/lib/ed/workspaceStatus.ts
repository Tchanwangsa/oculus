import type { WorkspaceState } from "@/lib/ed/workspace";
import type { SaveState } from "@/lib/ed/workspaceSaver";

/**
 * The workspace's status line, worded as Ed's: a write in flight first, then
 * a failed one (its message shows in an Alert), then work kept only on this
 * computer (the last save landed locally, or the socket is down), then the
 * connection, else everything saved.
 */
export function workspaceStatusText(
  connection: WorkspaceState,
  save: SaveState,
  computer: string,
): string {
  if (save.state === "saving") return "Saving…";
  if (save.state === "error") return "Not saved to Ed";
  if (connection === "offline" || (save.state === "saved" && save.where === "local")) {
    return `Offline — saved on this ${computer}`;
  }
  if (connection === "connecting") return "Connecting…";
  return "All changes saved";
}
