import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { versionTitle, type DocumentVersion } from "@/lib/notes/documentVersions";
import { shortcut } from "@/lib/platform";

/** The confirmations before Replace current and a checkpoint's delete. */
export function HistoryDialogs({
  selected,
  confirm,
  busy,
  onCancel,
  onReplace,
  onDelete,
}: {
  selected: DocumentVersion | null;
  confirm: "replace" | "delete" | null;
  busy: string | null;
  onCancel: () => void;
  onReplace: (v: DocumentVersion) => Promise<void>;
  onDelete: (v: DocumentVersion) => Promise<void>;
}) {
  return (
    <>
      <ConfirmDialog
        open={confirm === "replace" && !!selected}
        onCancel={onCancel}
        title={selected ? `Replace the note with ${versionTitle(selected)}?` : ""}
        description={`The note's current text is saved as a snapshot first, and ${shortcut("mod+Z")} in the note undoes the replace.`}
        confirmLabel="Replace"
        busyLabel="Replacing…"
        busy={busy === "replace"}
        onConfirm={async () => {
          if (selected) await onReplace(selected);
          onCancel();
        }}
      />
      <ConfirmDialog
        open={confirm === "delete" && !!selected}
        onCancel={onCancel}
        title={selected ? `Delete ${versionTitle(selected)}?` : ""}
        description="This saved version is gone for good. The note itself doesn't change."
        confirmLabel="Delete"
        busyLabel="Deleting…"
        busy={busy === "delete"}
        onConfirm={async () => {
          if (selected) await onDelete(selected);
          onCancel();
        }}
      />
    </>
  );
}
