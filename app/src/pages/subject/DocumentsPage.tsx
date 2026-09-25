import { useCallback, useEffect, useMemo, useState } from "react";
import { CircleNotch, NotePencil, Trash, Warning } from "@phosphor-icons/react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { FileRecency } from "@/components/files/FileRecency";
import { useSubject } from "@/layouts/SubjectLayout";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import type { DbFile } from "@/lib/db";
import { createDocument, deleteDocument, reconcileDocuments } from "@/lib/documents";
import { fmtSize, sqliteUtcToMs } from "@/lib/format";
import { filePagePath, fileTitle } from "@/lib/openFile";
import { navigateActive } from "@/lib/tabRouters";

/**
 * The student's own notes for a subject — markdown written here, kept as
 * files under `courses/<code>/documents/`, and from there ordinary library
 * files that ⌘K finds and the chat agent reads.
 *
 * The list is thin for the same reason Uploads is: the editor is the file
 * page (`DocumentEditor` hosted by `FilePage`), and a row is only the way
 * into it. What is this page's own is the reconcile on mount — the folder is
 * plain markdown on disk, and a note written there by hand deserves a row.
 */
export default function SubjectDocumentsPage() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);
  const documents = useMemo(
    () => [...byCategory.document].sort(byLastEdit),
    [byCategory.document],
  );

  const [creating, setCreating] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<DbFile | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Rows for what is on disk. The reconcile announces when it changed
  // anything, and `useSubjectFiles` reloads on that announcement.
  const { id: subjectId, code: subjectCode } = subject;
  useEffect(() => {
    let live = true;
    reconcileDocuments({ id: subjectId, code: subjectCode }).catch(
      (e) => live && setProblem(String(e)),
    );
    return () => {
      live = false;
    };
  }, [subjectId, subjectCode]);

  const create = useCallback(async () => {
    setCreating(true);
    setProblem(null);
    try {
      const row = await createDocument(subject);
      navigateActive(filePagePath(subject.id, row.relative_path));
    } catch (e) {
      setProblem(String(e));
    } finally {
      setCreating(false);
    }
  }, [subject]);

  const empty = !loading && documents.length === 0;

  return (
    <>
      <div className="page-scroll">
        <div className="mx-auto max-w-5xl px-6 py-5">
          <header className="mb-4 flex items-start justify-between gap-4">
            <div className="min-w-0">
              <h2 className="text-[13px] font-medium text-foreground">Your notes</h2>
              <p className="mt-0.5 text-[12px] text-muted-foreground">
                Markdown you write here. Notes are searchable, and the chat
                agent can read them alongside the rest of the subject.
              </p>
            </div>
            <Button size="sm" onClick={create} disabled={creating}>
              {creating ? (
                <CircleNotch className="animate-spin" aria-hidden />
              ) : (
                <NotePencil aria-hidden />
              )}
              New document
            </Button>
          </header>

          {problem && (
            <Alert variant="warning" className="mb-3 w-auto px-2.5 py-2">
              <Warning aria-hidden />
              <AlertDescription className="text-[11px] leading-snug">{problem}</AlertDescription>
            </Alert>
          )}

          {loading && documents.length === 0 ? (
            <div className="space-y-2">
              {Array.from({ length: 4 }).map((_, i) => (
                <Skeleton key={i} className="h-8 w-full" />
              ))}
            </div>
          ) : empty ? (
            <EmptyState onCreate={create} creating={creating} />
          ) : (
            <div className="divide-y divide-border-subtle overflow-hidden rounded-lg border border-border">
              {documents.map((f) => (
                <DocumentRow key={f.id} file={f} onDelete={() => setPendingDelete(f)} />
              ))}
            </div>
          )}
        </div>
      </div>

      <Dialog
        open={!!pendingDelete}
        onOpenChange={(open) => !open && !deleting && setPendingDelete(null)}
      >
        <DialogContent className="sm:max-w-sm" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Delete this document?</DialogTitle>
            <DialogDescription>
              {pendingDelete
                ? `“${fileTitle(pendingDelete)}” is deleted from your library. There is no trash to get it back from.`
                : ""}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              disabled={deleting}
              onClick={() => setPendingDelete(null)}
            >
              Keep it
            </Button>
            <Button
              variant="destructive"
              disabled={deleting}
              onClick={async () => {
                if (!pendingDelete) return;
                setDeleting(true);
                try {
                  await deleteDocument(pendingDelete);
                } catch (e) {
                  setProblem(String(e));
                } finally {
                  setPendingDelete(null);
                  setDeleting(false);
                }
              }}
            >
              {deleting && <CircleNotch className="animate-spin" aria-hidden />}
              {deleting ? "Deleting…" : "Delete"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

/** Most recently edited first; a note is the thing you were last writing. */
function byLastEdit(a: DbFile, b: DbFile): number {
  const at = (f: DbFile) => sqliteUtcToMs(f.modified_at) ?? sqliteUtcToMs(f.scraped_at) ?? 0;
  return at(b) - at(a) || fileTitle(a).localeCompare(fileTitle(b));
}

function EmptyState({ onCreate, creating }: { onCreate: () => void; creating: boolean }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border px-6 py-14 text-center">
      <NotePencil size={24} className="text-muted-foreground/40" aria-hidden />
      <div className="space-y-1">
        <p className="text-sm text-foreground">No notes yet</p>
        <p className="text-[12px] text-muted-foreground">
          Write in markdown. Each note is a file the subject's search and chat can see.
        </p>
      </div>
      <Button variant="outline" size="sm" onClick={onCreate} disabled={creating}>
        New document
      </Button>
    </div>
  );
}

function DocumentRow({ file, onDelete }: { file: DbFile; onDelete: () => void }) {
  const path = filePagePath(file.subject_id, file.relative_path);
  return (
    <div className="group flex items-center gap-3 px-3 py-2 transition-colors hover:bg-surface">
      {/* The page, not the peek: a note opens to be written in. `data-tab-href`
          gives ⌘-click a tab of its own (`lib/newTabClicks.ts`). */}
      <button
        data-tab-href={path}
        onClick={() => navigateActive(path)}
        className="flex min-w-0 flex-1 items-center gap-3 text-left"
      >
        <NotePencil size={14} className="shrink-0 opacity-60" aria-hidden />
        <span className="flex-1 truncate text-[12px] text-foreground">{fileTitle(file)}</span>
        {/* Fixed-width, right-aligned columns so every row lines up. */}
        <span className="w-17 shrink-0 text-right text-[11px] text-muted-foreground tabular-nums">
          {fmtSize(file.size_bytes)}
        </span>
        <span className="flex w-13 shrink-0 items-center justify-end">
          <FileRecency file={file} />
        </span>
      </button>

      <span className="flex w-3 shrink-0 items-center justify-center">
        <button
          onClick={onDelete}
          aria-label={`Delete ${fileTitle(file)}`}
          title="Delete this document"
          className="text-muted-foreground opacity-0 transition-[color,opacity] group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100"
        >
          <Trash size={12} aria-hidden />
        </button>
      </span>
    </div>
  );
}
