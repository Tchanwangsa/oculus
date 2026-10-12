import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useRef, useState } from "react";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { SyncRunFile, SyncRunSummary } from "@/lib/db";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { ACTION_LABEL } from "@/components/sync/history/constants";
import { FileLine } from "@/components/sync/history/FileLine";

export function RunFilesDialog({
  run,
  files,
  open,
  onOpenChange,
}: {
  run: SyncRunSummary;
  files: SyncRunFile[];
  open: boolean;
  onOpenChange: (v: boolean) => void;
}) {
  const groups = (["new", "updated", "unchanged"] as const)
    .map((action) => ({ action, files: files.filter((f) => f.action === action) }))
    .filter((g) => g.files.length > 0);
  // Radix mounts the content a render after `open`, so the scroller is held
  // in state too: its arrival re-runs the fade's effect.
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);
  useScrollFade(scrollRef, "y", scroller);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="text-sm">
            {fmtClock(sqliteUtcToMs(run.started_at), true)}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {files.length} file{files.length === 1 ? "" : "s"} checked in this run
          </DialogDescription>
        </DialogHeader>
        {/* Group headings scroll with their rows rather than stick: the edge
            fade masks the top, where a sticky heading would sit. The layer is
            promoted up front so icons don't step when the fade starts. */}
        <div
          ref={(el) => {
            scrollRef.current = el;
            setScroller(el);
          }}
          className="max-h-[55vh] overflow-y-auto will-change-transform"
        >
          {groups.map((g) => (
            <div key={g.action} className="mb-3 last:mb-0">
              <p className="py-1.5 text-[11px] font-medium text-muted-foreground">
                {ACTION_LABEL[g.action]} ({g.files.length})
              </p>
              <div className="divide-y divide-border-subtle">
                {g.files.map((f) => (
                  <FileLine key={f.id} file={f} />
                ))}
              </div>
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
