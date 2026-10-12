import type { WorkspaceStatus } from "@/lib/ed/workspace";
import type { SaveState } from "@/lib/ed/workspaceSaver";
import { workspaceStatusText } from "@/lib/ed/workspaceStatus";
import { isMac } from "@/lib/platform";
import { cn } from "@/lib/utils";

/** "Saving…", "All changes saved", "Offline — saved on this Mac",
 *  "Connecting…" (`workspaceStatusText`); Rust's reason for being offline
 *  is the tooltip. */
export function WorkspaceStatusLine({ connection, save }: { connection: WorkspaceStatus; save: SaveState }) {
  const text = workspaceStatusText(connection.state, save, isMac ? "Mac" : "PC");
  return (
    <span
      role="status"
      title={connection.state === "offline" ? (connection.reason ?? undefined) : undefined}
      className={cn(
        "shrink-0 truncate text-[11px]",
        save.state === "error" ? "text-destructive" : "text-muted-foreground",
      )}
    >
      {text}
    </span>
  );
}
