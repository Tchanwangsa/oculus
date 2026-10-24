import { useCallback, useEffect, useMemo, useState } from "react";
import { CaretDown, CircleNotch, FolderSimple, Notebook, NotePencil, Plus, Scribble, Trash, Warning } from "@phosphor-icons/react";
import { DropdownMenu } from "radix-ui";

import { SubjectPage } from "@/components/subjects/SubjectPage";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { EmptyState, ListCard, SkeletonRows } from "@/components/ui/layout/PageParts";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { FileRecency } from "@/components/files/FileRecency";
import { NotebookDialog } from "@/components/canvas/NotebookDialog";
import { useSubject } from "@/layouts/SubjectLayout";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import type { DbFile } from "@/lib/db";
import { createDocument, deleteDocument, reconcileDocuments } from "@/lib/notes/documents";
import { createCanvas, reconcileCanvases } from "@/lib/canvas/files";
import { canvasSessions } from "@/lib/canvas/sessionRuntime";
import { fmtSize, sqliteUtcToMs } from "@/lib/format/format";
import { filePagePath, fileTitle } from "@/lib/files/openFile";
import { navigateActive } from "@/lib/shell/tabRouters";

type DriveKind = "document" | "canvas";
const FILE_KINDS = {
  document: { label: "Note", icon: NotePencil, create: createDocument, remove: deleteDocument },
  canvas: { label: "Canvas", icon: Scribble, create: createCanvas, remove: (file: DbFile) => canvasSessions.open(file).remove() },
} satisfies Record<DriveKind, {
  label: string;
  icon: typeof NotePencil;
  create: (subject: { id: number; code: string }) => Promise<DbFile>;
  remove: (file: DbFile) => Promise<void>;
}>;
const FILTERS = [
  { value: "all", label: "All" },
  { value: "document", label: "Notes" },
  { value: "canvas", label: "Canvases" },
] as const;
type Filter = typeof FILTERS[number]["value"];

/** Notes and drawing canvases share a library; each opens its own file editor. */
export default function SubjectDrivePage() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);
  const [filter, setFilter] = useState<Filter>("all");
  const [creating, setCreating] = useState(false);
  const [notebookOpen, setNotebookOpen] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<DbFile | null>(null);
  const [deleting, setDeleting] = useState(false);
  const files = useMemo(() => [...byCategory.document, ...byCategory.canvas].sort(byLastEdit), [byCategory.document, byCategory.canvas]);
  const filtered = useMemo(() => filter === "all" ? files : files.filter(file => file.category === filter), [files, filter]);

  const { id: subjectId, code: subjectCode } = subject;
  useEffect(() => {
    let live = true;
    const identity = { id: subjectId, code: subjectCode };
    void Promise.allSettled([reconcileDocuments(identity), reconcileCanvases(identity)]).then(results => {
      const errors = results.flatMap(result => result.status === "rejected" ? [String(result.reason)] : []);
      if (live && errors.length) setProblem(errors.join("\n"));
    });
    return () => { live = false; };
  }, [subjectId, subjectCode]);

  const create = useCallback(async (kind: DriveKind) => {
    if (creating) return;
    setCreating(true);
    setProblem(null);
    try {
      const row = await FILE_KINDS[kind].create(subject);
      navigateActive(filePagePath(subject.id, row.relative_path));
    } catch (error) {
      setProblem(String(error));
    } finally {
      setCreating(false);
    }
  }, [creating, subject]);

  return (
    <>
      <SubjectPage>
        <header className="mb-4 flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h2 className="text-[13px] font-medium text-foreground">Drive</h2>
            <p className="mt-0.5 text-[12px] text-muted-foreground">Your notes, diagrams and sketches, together.</p>
          </div>
          <NewFileMenu busy={creating} onCreate={create} onNotebook={() => setNotebookOpen(true)} />
        </header>

        {problem && <Alert variant="warning" className="mb-3 w-auto px-2.5 py-2">
          <Warning aria-hidden />
          <AlertDescription className="whitespace-pre-line text-[11px] leading-snug">{problem}</AlertDescription>
        </Alert>}

        <div className="mb-4 -ml-2"><PillTabs tabs={FILTERS} value={filter} onChange={setFilter} /></div>

        {loading && !files.length ? <SkeletonRows count={4} /> : !filtered.length ? (
          <EmptyState
            icon={<FolderSimple size={24} className="text-muted-foreground/40" aria-hidden />}
            title={!files.length ? "No files yet" : filter === "document" ? "No notes yet" : "No canvases yet"}
            body={!files.length ? "Choose New file to start a note or a canvas." : filter === "document" ? "Create a note from the New file menu." : "Create a canvas from the New file menu."}
          >{null}</EmptyState>
        ) : (
          <>
            <div className="flex items-center gap-3 border-b border-border-subtle px-3 pb-2 text-[11px] text-muted-foreground">
              <span className="min-w-0 flex-1">Name</span>
              <span className="w-16 shrink-0">Type</span>
              <span className="w-14 shrink-0 text-right">Size</span>
              <span className="w-13 shrink-0 text-right">Opened</span>
              <span className="w-3 shrink-0" />
            </div>
            <ListCard>{filtered.map(file => <DriveRow key={file.id} file={file} onDelete={() => setPendingDelete(file)} />)}</ListCard>
          </>
        )}
      </SubjectPage>

      <NotebookDialog open={notebookOpen} onOpenChange={setNotebookOpen} subject={subject}
        onCreated={row => navigateActive(filePagePath(subject.id, row.relative_path))} />

      <ConfirmDialog
        open={!!pendingDelete}
        onCancel={() => setPendingDelete(null)}
        title={`Delete this ${pendingDelete?.category === "canvas" ? "canvas" : "note"}?`}
        description={pendingDelete ? `“${fileTitle(pendingDelete)}” is deleted from your library. There is no trash to get it back from.` : ""}
        confirmLabel="Delete" busyLabel="Deleting…" busy={deleting}
        onConfirm={async () => {
          if (!pendingDelete) return;
          setDeleting(true);
          setProblem(null);
          try { await FILE_KINDS[pendingDelete.category as DriveKind].remove(pendingDelete); }
          catch (error) { setProblem(String(error)); }
          finally { setPendingDelete(null); setDeleting(false); }
        }}
      />
    </>
  );
}

function NewFileMenu({ busy, onCreate, onNotebook }: { busy: boolean; onCreate: (kind: DriveKind) => void; onNotebook: () => void }) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button size="sm" disabled={busy}>
          {busy ? <CircleNotch className="animate-spin" aria-hidden /> : <Plus aria-hidden />}
          {busy ? "Creating…" : "New file"}<CaretDown aria-hidden />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="end" sideOffset={6} className="z-50 min-w-40 rounded-xl border border-border-subtle bg-popover p-1 text-popover-foreground shadow-lg outline-none">
          {(Object.keys(FILE_KINDS) as DriveKind[]).map(kind => {
            const { icon: Icon, label } = FILE_KINDS[kind];
            return <DropdownMenu.Item key={kind} onSelect={() => onCreate(kind)} className="flex cursor-pointer items-center gap-2 rounded-lg px-2.5 py-2 text-[12px] outline-none data-highlighted:bg-accent">
              <Icon size={15} className="text-reference-page" aria-hidden />{label}
            </DropdownMenu.Item>;
          })}
          <DropdownMenu.Item onSelect={onNotebook} className="flex cursor-pointer items-center gap-2 rounded-lg px-2.5 py-2 text-[12px] outline-none data-highlighted:bg-accent">
            <Notebook size={15} className="text-reference-page" aria-hidden />Notebook…
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function byLastEdit(a: DbFile, b: DbFile): number {
  const at = (file: DbFile) => sqliteUtcToMs(file.modified_at) ?? sqliteUtcToMs(file.scraped_at) ?? 0;
  return at(b) - at(a) || fileTitle(a).localeCompare(fileTitle(b));
}

function DriveRow({ file, onDelete }: { file: DbFile; onDelete: () => void }) {
  const path = filePagePath(file.subject_id, file.relative_path);
  const { icon: Icon, label } = FILE_KINDS[file.category as DriveKind];
  return (
    <div className="group flex items-center gap-3 px-3 py-2 transition-colors hover:bg-surface">
      <button type="button" data-tab-href={path} onClick={() => navigateActive(path)} className="flex min-w-0 flex-1 cursor-pointer items-center gap-3 rounded-sm text-left focus-visible:outline-2 focus-visible:outline-ring">
        <Icon size={14} className="shrink-0 text-reference-page" aria-hidden />
        <span className="min-w-0 flex-1 truncate text-[12px] text-foreground">{fileTitle(file)}</span>
        <span className="w-16 shrink-0 text-[11px] text-muted-foreground">{label}</span>
        <span className="w-14 shrink-0 text-right text-[11px] text-muted-foreground tabular-nums">{fmtSize(file.size_bytes)}</span>
        <span className="flex w-13 shrink-0 items-center justify-end"><FileRecency file={file} /></span>
      </button>
      <button type="button" onClick={onDelete} aria-label={`Delete ${fileTitle(file)}`} title={`Delete this ${label.toLowerCase()}`} className="flex w-3 shrink-0 cursor-pointer items-center justify-center text-muted-foreground opacity-0 transition-[color,opacity] will-change-[opacity] group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100">
        <Trash size={12} aria-hidden />
      </button>
    </div>
  );
}
