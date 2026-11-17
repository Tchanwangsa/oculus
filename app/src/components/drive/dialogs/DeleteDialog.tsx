import { useEffect, useState } from "react";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { deletePreview, driveErrorText } from "@/lib/drive/commands";
import type { DriveItem } from "@/lib/drive/items";
import { baseName } from "@/lib/drive/paths";

/** Files listed by name before "and N more". */
const SHOWN = 6;

/**
 * "Delete these?" for the student's own entries. It lists every file the
 * delete removes (`delete_preview`, so a folder says what is inside it)
 * before the student confirms; there is no trash to get them back from.
 */
export function DeleteDialog({ items, onCancel, onConfirm }: {
  items: DriveItem[] | null;
  onCancel: () => void;
  onConfirm: (items: DriveItem[]) => Promise<void>;
}) {
  const [files, setFiles] = useState<string[] | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const key = items?.map((i) => i.key).join("\n") ?? "";

  useEffect(() => {
    setFiles(null);
    setProblem(null);
    if (!key) return;
    let live = true;
    deletePreview(key.split("\n"))
      .then((list) => live && setFiles(list))
      .catch((e) => live && setProblem(driveErrorText(e)));
    return () => { live = false; };
  }, [key]);

  const one = items?.length === 1 ? items[0] : null;
  const title = one ? `Delete “${one.name}”?` : `Delete ${items?.length ?? 0} items?`;
  return (
    <ConfirmDialog
      open={!!items}
      onCancel={onCancel}
      title={title}
      confirmLabel="Delete"
      busyLabel="Deleting…"
      busy={busy || (files == null && !problem)}
      description={
        problem ? (
          <span className="text-destructive">{problem}</span>
        ) : files == null ? (
          "Finding what this removes…"
        ) : (
          <>
            <span className="block">
              {files.length === 1 ? "This file is" : `These ${files.length} files are`} deleted from your library, with
              what was read out of them. There is no trash to get them back from.
            </span>
            <span className="mt-2 block max-h-40 overflow-y-auto rounded-md bg-surface px-2.5 py-1.5 text-[12px] text-foreground">
              {files.slice(0, SHOWN).map((f) => <span key={f} className="block truncate" title={f}>{baseName(f)}</span>)}
              {files.length > SHOWN && <span className="block text-muted-foreground">and {files.length - SHOWN} more</span>}
              {files.length === 0 && <span className="block text-muted-foreground">Empty folders only</span>}
            </span>
          </>
        )
      }
      onConfirm={async () => {
        if (!items || problem) return;
        setBusy(true);
        try {
          await onConfirm(items);
        } finally {
          setBusy(false);
        }
      }}
    />
  );
}
