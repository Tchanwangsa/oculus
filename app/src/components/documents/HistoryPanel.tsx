import { memo, useCallback, useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import {
  ArrowCounterClockwise,
  BookmarkSimple,
  CircleNotch,
  ClockCounterClockwise,
  CopySimple,
  PencilSimple,
  Trash,
  X,
} from "@phosphor-icons/react";

import { fileMarkdownPlugins } from "@/components/files/FileMarkdown";
import { useLibraryMdComponents } from "@/components/files/FileViewer";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { Input } from "@/components/ui/input";
import { SkeletonRows } from "@/components/ui/PageParts";
import { ResizeHandle } from "@/components/ui/ResizeHandle";
import { useWindowEvent } from "@/hooks/useEvents";
import { useNow } from "@/hooks/useNow";
import { useResizablePanel } from "@/hooks/useResizablePanel";
import { useScrollFade } from "@/hooks/useScrollFade";
import type { DbFile } from "@/lib/db";
import {
  DOCUMENT_VERSIONS_EVENT,
  deleteVersion,
  listVersions,
  relabelVersion,
  restoreAsCopy,
  snapshot,
  versionText,
  versionTitle,
  type DocumentVersion,
  type VersionKind,
} from "@/lib/documentVersions";
import { fmtFullStamp, fmtRecent, sqliteUtcToMs } from "@/lib/format";
import { hasMath, normalizeMath } from "@/lib/mathMarkdown";
import { filePagePath, openFileSmart } from "@/lib/openFile";
import { copyAsMarkdown, dragAsMarkdown } from "@/lib/selectionMarkdown";
import { cn } from "@/lib/utils";
import { useTabStore } from "@/stores/tabStore";

const PANEL = {
  defaultWidth: 320,
  minWidth: 260,
  maxWidth: 640,
  side: "right",
  // Closing is the header's toggle, so a drag never folds it.
  collapseThreshold: 0,
  storageKey: "oculus-document-history",
} as const;

/** What a snapshot row says when it has no label of its own. */
const SNAPSHOT_HINT: Record<Exclude<VersionKind, "checkpoint">, string> = {
  auto: "Autosave snapshot",
  external: "Changed outside the editor",
  restore: "Before a restore",
};

/** Leading YAML frontmatter, which markdown would read as a rule and a
 *  heading; the preview shows it as the YAML it is. */
const FRONTMATTER = /^---\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/;

/**
 * A note's saved versions (`@/lib/documentVersions`), docked right of the note
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

  /** Null while the first read is out. */
  const [versions, setVersions] = useState<DocumentVersion[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const selected = versions?.find((v) => v.id === selectedId) ?? null;

  // Only the newest read lands, so a burst of events can't paint an old list.
  const readSeq = useRef(0);
  const load = useCallback(() => {
    const seq = ++readSeq.current;
    listVersions(fileId)
      .then((rows) => {
        if (seq !== readSeq.current) return;
        setVersions(rows);
        setListError(null);
      })
      .catch((e) => seq === readSeq.current && setListError(String(e)));
  }, [fileId]);
  useEffect(() => {
    load();
    return () => {
      readSeq.current++;
    };
  }, [load]);
  useWindowEvent(DOCUMENT_VERSIONS_EVENT, (e) => {
    const id = (e as CustomEvent<{ fileId?: number }>).detail?.fileId;
    if (id == null || id === fileId) load();
  });

  // A version's text never changes, so each is read once.
  const texts = useRef(new Map<number, string>());
  const [preview, setPreview] = useState<{ id: number; text: string } | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  useEffect(() => {
    setPreviewError(null);
    if (selectedId == null) return;
    const cached = texts.current.get(selectedId);
    if (cached != null) {
      setPreview({ id: selectedId, text: cached });
      return;
    }
    let live = true;
    versionText(selectedId)
      .then((text) => {
        texts.current.set(selectedId, text);
        if (live) setPreview({ id: selectedId, text });
      })
      .catch((e) => live && setPreviewError(String(e)));
    return () => {
      live = false;
    };
  }, [selectedId]);
  const textOf = async (v: DocumentVersion): Promise<string> => {
    const cached = texts.current.get(v.id);
    if (cached != null) return cached;
    const text = await versionText(v.id);
    texts.current.set(v.id, text);
    return text;
  };

  /** The action running on the selected version, and the last one's failure. */
  const [busy, setBusy] = useState<"copy" | "replace" | "label" | "delete" | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<"replace" | "delete" | null>(null);
  const [renaming, setRenaming] = useState(false);

  // A different row starts clean.
  useEffect(() => {
    setActionError(null);
    setRenaming(false);
  }, [selectedId]);

  const run = async (kind: NonNullable<typeof busy>, action: () => Promise<void>) => {
    setBusy(kind);
    setActionError(null);
    try {
      await action();
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const restoreCopy = (v: DocumentVersion) =>
    run("copy", async () => {
      if (!subject) throw new Error("The note's subject hasn't loaded yet.");
      const row = await restoreAsCopy(subject, file, v);
      useTabStore.getState().addTab(filePagePath(row.subject_id, row.relative_path));
    });

  const replace = (v: DocumentVersion) =>
    run("replace", async () => {
      const text = await textOf(v);
      const current = currentText();
      if (current == null) throw new Error("The note isn't open in an editor.");
      if (current === text) return;
      await snapshot(fileId, current, "restore", `Before restoring ${versionTitle(v)}`);
      if (!replaceText(text)) throw new Error("The note isn't open in an editor.");
    });

  const relabel = (v: DocumentVersion, label: string) => {
    setRenaming(false);
    const next = label.trim() || null;
    if (next === (v.label ?? null)) return;
    void run("label", () => relabelVersion(v.id, next));
  };

  const remove = (v: DocumentVersion) =>
    run("delete", async () => {
      await deleteVersion(v.id);
      texts.current.delete(v.id);
      setSelectedId((id) => (id === v.id ? null : id));
    });

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

/** One version: its title, then when (checkpoints) or why (snapshots), and
 *  its length. Checkpoints — the student's own — read stronger. */
function VersionRow({
  version: v,
  now,
  selected,
  onSelect,
}: {
  version: DocumentVersion;
  now: Date;
  selected: boolean;
  onSelect: () => void;
}) {
  const at = sqliteUtcToMs(v.created_at);
  const checkpoint = v.kind === "checkpoint";
  const detail =
    v.kind !== "checkpoint" ? (v.label ?? SNAPSHOT_HINT[v.kind])
    : at != null ? fmtRecent(at, now)
    : null;
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      title={at != null ? fmtFullStamp(at) : undefined}
      className={cn(
        "flex w-full cursor-pointer items-start gap-2 px-3 py-1.5 text-left transition-colors",
        selected ? "bg-accent" : "hover:bg-surface",
      )}
    >
      {checkpoint ? (
        <BookmarkSimple size={12} weight="fill" className="mt-0.5 shrink-0 text-brand" aria-hidden />
      ) : (
        <ClockCounterClockwise size={12} className="mt-0.5 shrink-0 text-muted-foreground/60" aria-hidden />
      )}
      <span className="min-w-0 flex-1">
        <span
          className={cn(
            "block truncate text-[12px]",
            checkpoint ? "font-medium text-foreground" : "text-muted-foreground",
          )}
        >
          {versionTitle(v)}
        </span>
        <span className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
          {detail && <span className="min-w-0 truncate">{detail}</span>}
          {detail && (
            <span aria-hidden className="shrink-0 text-muted-foreground/50">
              ·
            </span>
          )}
          <span className="shrink-0 tabular-nums">{v.length.toLocaleString()} chars</span>
        </span>
      </span>
    </button>
  );
}

/** The selected checkpoint's label, edited in its row: Enter or leaving the
 *  field saves (blank clears it), Esc keeps the old one. */
function LabelField({
  version,
  onDone,
  onCancel,
}: {
  version: DocumentVersion;
  onDone: (label: string) => void;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState(version.label ?? "");
  // Enter and Esc unmount the field; its blur must not commit a second time.
  const settled = useRef(false);
  const finish = (commit: boolean) => {
    if (settled.current) return;
    settled.current = true;
    if (commit) onDone(draft);
    else onCancel();
  };
  return (
    <div className="flex items-center gap-2 bg-accent px-3 py-1.5">
      <BookmarkSimple size={12} weight="fill" className="shrink-0 text-brand" aria-hidden />
      <Input
        autoFocus
        value={draft}
        placeholder="Name this version"
        aria-label="Version name"
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => finish(true)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            finish(true);
          } else if (e.key === "Escape") {
            e.preventDefault();
            finish(false);
          }
        }}
        className="h-7 bg-card text-[12px]"
      />
    </div>
  );
}

/** A version's text as the library's read-only markdown, scaled to the panel
 *  (`.md-compact`); links and pictures resolve against the note's folder. */
const VersionPreview = memo(function VersionPreview({
  text,
  file,
  files,
}: {
  text: string;
  file: DbFile;
  files: DbFile[];
}) {
  const components = useLibraryMdComponents(file, files, openFileSmart);
  const front = FRONTMATTER.exec(text);
  const source = front ? "~~~yaml\n" + front[1] + "\n~~~\n\n" + text.slice(front[0].length) : text;
  if (!source.trim()) {
    return <p className="text-[12px] text-muted-foreground">This version is empty.</p>;
  }
  return (
    <article className="md-compact" onCopy={copyAsMarkdown} onDragStart={dragAsMarkdown}>
      <ReactMarkdown {...fileMarkdownPlugins(source)} components={components}>
        {hasMath(source) ? normalizeMath(source) : source}
      </ReactMarkdown>
    </article>
  );
});

function PanelError({ message }: { message: string }) {
  return (
    <Alert variant="destructive" className="mx-3 my-2 w-auto px-2.5 py-2">
      <AlertDescription className="text-[11px] leading-snug break-words">{message}</AlertDescription>
    </Alert>
  );
}
