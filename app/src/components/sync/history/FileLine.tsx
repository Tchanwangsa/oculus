import { Badge } from "@/components/ui/badge";
import type { SyncRunFile } from "@/lib/db";
import { displayCode, fmtSize } from "@/lib/format/format";
import { fileIconFor } from "@/lib/files/fileTypes";
import { ACTION_LABEL, ACTION_VARIANT } from "@/components/sync/history/constants";

export function FileLine({ file }: { file: SyncRunFile }) {
  const name = file.relative_path.split("/").pop() ?? file.relative_path;
  const Icon = fileIconFor(name);
  return (
    <div className="flex items-center gap-2.5 py-1.5 min-w-0">
      <Icon size={13} className="shrink-0 text-muted-foreground/70" />
      <span className="text-xs text-foreground truncate">{name}</span>
      <span className="text-[11px] text-muted-foreground shrink-0">
        {file.subject_code ? displayCode(file.subject_code) : ""}
      </span>
      <span className="flex-1" />
      <span className="text-[11px] text-muted-foreground tabular-nums shrink-0">
        {fmtSize(file.size_bytes)}
      </span>
      <Badge variant={ACTION_VARIANT[file.action]} className="text-[11px] shrink-0">
        {ACTION_LABEL[file.action]}
      </Badge>
    </div>
  );
}
