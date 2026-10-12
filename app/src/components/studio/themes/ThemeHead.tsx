import { useState } from "react";
import { ArrowCounterClockwise, CopySimple, Trash } from "@phosphor-icons/react";

import { OwnVersionActions } from "@/components/studio/components/OwnVersionActions";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { displayCode } from "@/lib/format/format";
import type { ThemeInfo } from "@/lib/studio/themes";

/** Where the theme comes from, said plainly under its name. */
function levelText(info: ThemeInfo, subject: string | null): string {
  if (info.level === "bundled") return "Built in · read-only, duplicate it to change it";
  if (info.level === "global") return subject ? "From the global library · a save changes it for every subject" : "Global library";
  const code = subject ? displayCode(subject) : "the subject";
  return info.shadows ? `Own version in ${code}` : `${code}'s library`;
}

/**
 * The theme's name and where it lives, and what can be done with it: Save or
 * Revert an edit (a built-in theme saves as a copy), Undo the last save,
 * Duplicate, an own version for the subject, and Delete for a library theme.
 */
export function ThemeHead({
  info,
  subject,
  dirty,
  saving,
  canUndo,
  onSave,
  onRevert,
  onUndo,
  onDuplicate,
  onDelete,
  onLibraryChanged,
}: {
  info: ThemeInfo;
  subject: string | null;
  dirty: boolean;
  saving: boolean;
  /** The theme's level has a snapshot to go back to. */
  canUndo: boolean;
  onSave: () => void;
  onRevert: () => void;
  onUndo: () => void;
  onDuplicate: () => void;
  onDelete: () => Promise<void>;
  onLibraryChanged: () => void;
}) {
  const [confirm, setConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);
  // A subject's own version is removed with its own action, which says what
  // comes back; a theme that replaces nothing is deleted.
  const deletable =
    (info.level === "global" && subject == null) || (info.level === "subject" && info.shadows == null);
  const readOnly = info.level === "bundled";

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-[18px] font-semibold leading-tight text-foreground">{info.name}</h2>
          <p className="text-[12px] text-muted-foreground">{levelText(info, subject)}</p>
        </div>
        {dirty && (
          <Button type="button" variant="ghost" size="sm" disabled={saving} onClick={onRevert}>
            Revert
          </Button>
        )}
        {dirty && (
          <Button type="button" size="sm" disabled={saving} onClick={onSave}>
            {readOnly ? "Save as a copy" : "Save"}
          </Button>
        )}
        {!dirty && canUndo && (
          <Button type="button" variant="outline" size="sm" title="Put back the text from before the last save" onClick={onUndo}>
            <ArrowCounterClockwise size={13} />
            Undo
          </Button>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <Button type="button" variant="outline" size="xs" onClick={onDuplicate}>
          <CopySimple size={12} />
          Duplicate
        </Button>
        <OwnVersionActions
          kind="theme"
          name={info.name}
          level={info.level}
          shadows={info.shadows != null}
          subject={subject}
          onDone={onLibraryChanged}
        />
        {deletable && (
          <Button type="button" variant="ghost" size="xs" onClick={() => setConfirm(true)}>
            <Trash size={12} />
            Delete
          </Button>
        )}
      </div>
      <ConfirmDialog
        open={confirm}
        onCancel={() => setConfirm(false)}
        onConfirm={() => {
          setDeleting(true);
          void onDelete().finally(() => {
            setDeleting(false);
            setConfirm(false);
          });
        }}
        title={`Delete ${info.name}?`}
        description="Videos that name it stop loading until they pick another theme. Its text stays in the library's history."
        confirmLabel="Delete"
        busyLabel="Deleting"
        busy={deleting}
      />
    </div>
  );
}
