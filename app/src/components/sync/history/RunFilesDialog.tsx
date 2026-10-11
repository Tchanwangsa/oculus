import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="text-sm">
            Files — {fmtClock(sqliteUtcToMs(run.started_at), true)}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {files.length} file{files.length === 1 ? "" : "s"} touched in this run
          </DialogDescription>
        </DialogHeader>
        <div className="max-h-[55vh] overflow-y-auto -mx-1 px-1">
          {groups.map((g) => (
            <div key={g.action} className="mb-3 last:mb-0">
              <p className="text-[11px] font-medium text-muted-foreground py-1.5 sticky top-0 bg-background">
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
