import type { WorkspaceStatus } from "@/lib/ed/workspace";
import type { SaveState } from "@/lib/ed/workspaceSaver";
import { workspaceStatusText } from "@/lib/ed/workspaceStatus";
import { isMac } from "@/lib/platform";
import { cn } from "@/lib/utils";

/** "Saving…", "All changes saved", "Offline — saved on this Mac",
 *  "Connecting…" (`workspaceStatusText`); offline, Rust's reason sits under
 *  it in dim text. */
export function WorkspaceStatusLine({ connection, save }: { connection: WorkspaceStatus; save: SaveState }) {
  const text = workspaceStatusText(connection.state, save, isMac ? "Mac" : "PC");
  const reason = connection.state === "offline" ? connection.reason : null;
  return (
    <span role="status" title={reason ?? undefined} className="flex min-w-0 max-w-64 shrink-0 flex-col items-end leading-tight">
      <span
        className={cn("max-w-full truncate text-[11px]", save.state === "error" ? "text-destructive" : "text-muted-foreground")}
      >
        {text}
      </span>
      {reason && <span className="max-w-full truncate text-[10px] text-muted-foreground/70">{reason}</span>}
    </span>
  );
}
