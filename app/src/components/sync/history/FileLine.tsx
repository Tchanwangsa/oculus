import { Badge } from "@/components/ui/badge";
import type { SyncRunFile } from "@/lib/db";
import { displayCode, fmtSize } from "@/lib/format/format";
import { pathFile } from "@/lib/files/openFile";
import { fileReferenceAppearance } from "@/lib/files/referenceAppearance";
import { cn } from "@/lib/utils";
import { ACTION_LABEL, ACTION_VARIANT } from "@/components/sync/history/constants";

export function FileLine({ file }: { file: SyncRunFile }) {
  const name = file.relative_path.split("/").pop() ?? file.relative_path;
  // The file's reference colour: PDFs red, lessons, threads and pages their own.
  const { Icon, tone } = fileReferenceAppearance(
    pathFile(file.relative_path),
    file.relative_path.replace(/^courses\/[^/]+\//, ""),
  );
  return (
    <div className="flex items-center gap-2.5 py-1.5 min-w-0">
      <Icon size={13} className={cn("shrink-0", tone)} />
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
