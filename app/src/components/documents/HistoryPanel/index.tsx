import { useRef } from "react";
import { ArrowCounterClockwise, CircleNotch, CopySimple, PencilSimple, Trash, X } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { useNow } from "@/hooks/ui/useNow";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { DbFile } from "@/lib/db";
import { versionTitle } from "@/lib/notes/documentVersions";
import { cn } from "@/lib/utils";

import { PANEL } from "./constants";
import { LabelField } from "./LabelField";
import { PanelError } from "./PanelError";
import { useVersionActions } from "./useVersionActions";
import { useVersionList } from "./useVersionList";
import { useVersionPreview } from "./useVersionPreview";
import { VersionPreview } from "./VersionPreview";
import { VersionRow } from "./VersionRow";

/**
 * A note's saved versions (`@/lib/notes/documentVersions`), docked right of the note
 * inside the editor, so the note stays beside what is being compared. Newest
 * first; a row previews its text read-only, and the version can come back as
 * a new note (the default) or replace the open one — after a `restore`
 * snapshot, as one undoable edit through the editor so it saves like typing
 * and reaches the note's other views. Checkpoints can be renamed or deleted;
 * snapshots are the app's to prune.
 */
export function HistoryPanel({
  file,
  files,
  subject,
  currentText,
  replaceText,
  onClose,
}: {
  file: DbFile;
  /** The subject's files, for the preview's links. */
  files: DbFile[];
  /** Null while the subject list loads; a copy needs it. */
  subject: { id: number; code: string } | null;
  /** The editor's text now, or null with no editor. */
  currentText: () => string | null;
  /** Swap the editor's whole text as one undoable edit; false with no editor. */
  replaceText: (text: string) => boolean;
  onClose: () => void;
}) {
  const panel = useResizablePanel(PANEL);
  const now = useNow();
  const fileId = file.id;

  const { versions, listError, selectedId, setSelectedId, selected } = useVersionList(fileId);
  const { texts, preview, previewError, textOf } = useVersionPreview(selectedId);
  const { busy, actionError, confirm, setConfirm, renaming, setRenaming, restoreCopy, replace, relabel, remove } =
    useVersionActions({ file, subject, selectedId, setSelectedId, texts, textOf, currentText, replaceText });

  const listRef = useRef<HTMLDivElement>(null);
  useScrollFade(listRef, "y", versions);
  const previewRef = useRef<HTMLDivElement>(null);
  useScrollFade(previewRef, "y", selected?.id);

  return (
    <>
      {/* On the seam: negative margins cost no layout width. */}
      <ResizeHandle
        onMouseDown={panel.onMouseDown}
        dragging={panel.dragging}
        label="Resize history"
        className="-mx-0.5"
      />
      <aside
        aria-label="Version history"
        // A narrow pane keeps most of its width for the note.
        style={{ width: panel.width, maxWidth: "60%" }}
        className="flex min-h-0 shrink-0 flex-col border-l border-border-subtle bg-card"
      >
        <div className="flex h-9 shrink-0 items-center gap-2 border-b border-border-subtle pr-1.5 pl-3">
          <h2 className="min-w-0 flex-1 truncate text-[12px] font-medium text-foreground">History</h2>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Close history"
            title="Close history"
            className="text-muted-foreground"
            onClick={onClose}
          >
            <X size={13} />
          </Button>
        </div>

        {listError && <PanelError message={listError} />}

        {versions === null ? (
          !listError && <SkeletonRows count={4} rowClassName="h-10" className="p-3" />
        ) : versions.length === 0 ? (
          <div className="px-4 py-8 text-center">
            <p className="text-[13px] text-foreground">No versions yet</p>
            <p className="mt-1 text-[12px] text-muted-foreground">
              Save a version (⇧⌘S) to keep the note as it is now. Snapshots are
              also taken as you write.
            </p>
          </div>
        ) : (
          <div
            ref={listRef}
            role="listbox"
            aria-label="Versions"
            className={cn(
              "min-h-0 overflow-y-auto",
              selected ? "max-h-[40%] shrink-0" : "flex-1",
            )}
          >
            {/* One child, so the fade's observer sees rows come and go. */}
            <div className="py-1">
              {versions.map((v) =>
                renaming && v.id === selected?.id ? (
                  <LabelField
                    key={v.id}
                    version={v}
                    onDone={(label) => relabel(v, label)}
                    onCancel={() => setRenaming(false)}
                  />
                ) : (
                  <VersionRow
                    key={v.id}
                    version={v}
                    now={now}
                    selected={v.id === selectedId}
                    onSelect={() => setSelectedId(v.id === selectedId ? null : v.id)}
                  />
                ),
              )}
            </div>
          </div>
        )}

        {selected && (
          <>
            <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-y border-border-subtle px-3 py-2">
              <Button
                size="xs"
                disabled={busy !== null || !subject}
                onClick={() => void restoreCopy(selected)}
                title="Open this version as a new note"
              >
                {busy === "copy" ? <CircleNotch className="animate-spin" /> : <CopySimple />}
                Restore as copy
              </Button>
              <Button
                variant="outline"
                size="xs"
                disabled={busy !== null}
                onClick={() => setConfirm("replace")}
                title="Replace the note's text with this version"
              >
                <ArrowCounterClockwise />
                Replace current
              </Button>
              {selected.kind === "checkpoint" && (
                <span className="ml-auto flex items-center gap-0.5">
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label="Rename version"
                    title="Rename"
                    disabled={busy !== null}
                    className="text-muted-foreground"
                    onClick={() => setRenaming(true)}
                  >
                    <PencilSimple size={13} />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label="Delete version"
                    title="Delete"
                    disabled={busy !== null}
                    className="text-muted-foreground hover:text-destructive"
                    onClick={() => setConfirm("delete")}
                  >
                    <Trash size={13} />
                  </Button>
                </span>
              )}
            </div>
            {actionError && <PanelError message={actionError} />}
            <div ref={previewRef} className="min-h-0 flex-1 overflow-y-auto">
              {/* One child that grows as the text lands, for the fade. */}
              <div className="px-4 py-3">
                {previewError ? (
                  <p className="text-[11px] text-destructive">{previewError}</p>
                ) : preview?.id === selected.id ? (
                  <VersionPreview text={preview.text} file={file} files={files} />
                ) : (
                  <SkeletonRows count={3} rowClassName="h-4" />
                )}
              </div>
            </div>
          </>
        )}
      </aside>

      <ConfirmDialog
        open={confirm === "replace" && !!selected}
        onCancel={() => setConfirm(null)}
        title={selected ? `Replace the note with ${versionTitle(selected)}?` : ""}
        description="The note's current text is saved as a snapshot first, and ⌘Z in the note undoes the replace."
        confirmLabel="Replace"
        busyLabel="Replacing…"
        busy={busy === "replace"}
        onConfirm={async () => {
          if (selected) await replace(selected);
          setConfirm(null);
        }}
      />
      <ConfirmDialog
        open={confirm === "delete" && !!selected}
        onCancel={() => setConfirm(null)}
        title={selected ? `Delete ${versionTitle(selected)}?` : ""}
        description="This saved version is gone for good. The note itself doesn't change."
        confirmLabel="Delete"
        busyLabel="Deleting…"
        busy={busy === "delete"}
        onConfirm={async () => {
          if (selected) await remove(selected);
          setConfirm(null);
        }}
      />
    </>
  );
}
