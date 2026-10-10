import { SubjectPage } from "@/components/subjects/SubjectPage";
import { useCallback, useEffect, useMemo, useState } from "react";
import { CircleNotch, NotePencil, Trash, Warning } from "@phosphor-icons/react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { FileRecency } from "@/components/files/FileRecency";
import { useSubject } from "@/layouts/SubjectLayout";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import type { DbFile } from "@/lib/db";
import { createDocument, deleteDocument, reconcileDocuments } from "@/lib/notes/documents";
import { fmtSize, sqliteUtcToMs } from "@/lib/format/format";
import { filePagePath, fileTitle } from "@/lib/files/openFile";
import { navigateActive } from "@/lib/shell/tabRouters";
import { EmptyState, ListCard, SkeletonRows } from "@/components/ui/layout/PageParts";

/**
 * The student's own markdown notes for a subject, kept under
 * `courses/<code>/documents/` as ordinary library files. The editor is the
 * file page (`DocumentEditor` in `FilePage`); a row is only the way into it.
 * On mount the folder is reconciled so a note written there by hand gets a row.
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

  // The reconcile announces any change, and `useSubjectFiles` reloads on it.
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
      <SubjectPage>
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
          <SkeletonRows count={4} />
        ) : empty ? (
          <EmptyState
            icon={<NotePencil size={24} className="text-muted-foreground/40" aria-hidden />}
            title="No notes yet"
            body="Write in markdown. Each note is a file the subject's search and chat can see."
          >
            <Button variant="outline" size="sm" onClick={create} disabled={creating}>
              New document
            </Button>
          </EmptyState>
        ) : (
          <ListCard>
            {documents.map((f) => (
              <DocumentRow key={f.id} file={f} onDelete={() => setPendingDelete(f)} />
            ))}
          </ListCard>
        )}
      </SubjectPage>

      <ConfirmDialog
        open={!!pendingDelete}
        onCancel={() => setPendingDelete(null)}
        title="Delete this document?"
        description={
          pendingDelete
            ? `“${fileTitle(pendingDelete)}” is deleted from your library. There is no trash to get it back from.`
            : ""
        }
        confirmLabel="Delete"
        busyLabel="Deleting…"
        busy={deleting}
        onConfirm={async () => {
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
      />
    </>
  );
}

/** Most recently edited first; a note is the thing you were last writing. */
function byLastEdit(a: DbFile, b: DbFile): number {
  const at = (f: DbFile) => sqliteUtcToMs(f.modified_at) ?? sqliteUtcToMs(f.scraped_at) ?? 0;
  return at(b) - at(a) || fileTitle(a).localeCompare(fileTitle(b));
}

function DocumentRow({ file, onDelete }: { file: DbFile; onDelete: () => void }) {
  const path = filePagePath(file.subject_id, file.relative_path);
  return (
    <div className="group flex items-center gap-3 px-3 py-2 transition-colors hover:bg-surface">
      {/* The page, not the side panel: a note opens to be written in. */}
      <button
        data-tab-href={path}
        onClick={() => navigateActive(path)}
        className="flex min-w-0 flex-1 items-center gap-3 text-left"
      >
        <NotePencil size={14} className="shrink-0 opacity-60" aria-hidden />
        <span className="flex-1 truncate text-[12px] text-foreground">{fileTitle(file)}</span>
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
          className="text-muted-foreground opacity-0 transition-[color,opacity] will-change-[opacity] group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100"
        >
          <Trash size={12} aria-hidden />
        </button>
      </span>
    </div>
  );
}
