import { useMemo, useState } from "react";
import { Plus } from "@phosphor-icons/react";
import { PILL } from "@/components/projects/fields/pill";
import { useTaskFiles } from "@/hooks/data/useTaskFiles";
import { addTaskFile, removeTaskFile } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";
import { AttachFilePicker } from "./AttachFilePicker";
import { FileChip } from "./FileChip";

/** The task's related files: a chip per linked library file, then Attach file.
 *  The heading and the Attach chip always show. */
export function TaskFiles({ taskId }: { taskId: number }) {
  const { files } = useTaskFiles(taskId);
  const attachedIds = useMemo(() => new Set(files.map((f) => f.file_id)), [files]);
  const [error, setError] = useState<string | null>(null);

  return (
    <section className="mt-8">
      <h2 className="text-[13px] font-semibold tracking-tight text-foreground">Related files</h2>
      <div className="mt-2.5 flex flex-wrap items-center gap-2">
        {files.map((file) => (
          <FileChip
            key={file.file_id}
            file={file}
            onRemove={() => {
              setError(null);
              removeTaskFile(taskId, file.file_id).catch((e) =>
                setError(e instanceof Error ? e.message : String(e)),
              );
            }}
          />
        ))}
        <AttachFilePicker
          attachedIds={attachedIds}
          onAttach={async (file) => {
            setError(null);
            await addTaskFile(taskId, file.file_id);
          }}
        >
          <button type="button" className={cn(PILL, "h-8 text-muted-foreground")}>
            <Plus size={12} weight="bold" className="shrink-0" />
            Attach file
          </button>
        </AttachFilePicker>
      </div>
      {error && <p className="mt-1.5 text-[11px] text-destructive">{error}</p>}
    </section>
  );
}
