import { SubjectPage } from "@/components/subjects/SubjectPage";
import { useMemo, useState } from "react";
import { CircleNotch, Trash, UploadSimple, Warning } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useParseStore } from "@/stores/parseStore";
import { useReconcileParseStatus } from "@/hooks/useReconcileParseStatus";
import type { DbFile } from "@/lib/db";
import { fileIconFor, isPdfBacked } from "@/lib/fileTypes";
import { fmtSize } from "@/lib/format";
import { openFileSmart } from "@/lib/openFile";
import { useFilesTab } from "@/pages/subject/FilesPage";
import { UPLOADS_CHANGED_EVENT, removeUpload } from "@/lib/uploads";
import { useWindowEvent } from "@/hooks/useEvents";
import { EmptyState, ListCard, SkeletonRows } from "@/components/ui/PageParts";

/**
 * A subject's own files — material that belongs to the subject but was never
 * on Canvas. Once Rust has copied the bytes into `courses/<code>/uploads/`, an
 * upload is an ordinary library file, so the pipeline does the rest.
 *
 * The adding (picker, copy, "Adding…" rows, problems) belongs to the Files tab
 * (`useUploadImport` via `useFilesTab()`), because a Finder drop lands on
 * whichever sub-tab is showing and its rows must outlive a switch to this one.
 */
export default function SubjectUploadsPage() {
  const subject = useFilesTab();
  const { importing, problems, choose, report, busy } = subject.upload;
  const { byCategory, loading, reload } = useSubjectFiles(subject.id);
  const uploads = byCategory.upload;

  const [pendingDelete, setPendingDelete] = useState<DbFile | null>(null);
  const [deleting, setDeleting] = useState(false);

  const liveStatuses = useParseStore((s) => s.statuses);

  useWindowEvent(UPLOADS_CHANGED_EVENT, reload);

  useReconcileParseStatus(uploads);

  const empty = !loading && uploads.length === 0 && !busy;

  return (
    <>
      <SubjectPage>
        <header className="mb-4 flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h2 className="text-[13px] font-medium text-foreground">Your files</h2>
            <p className="mt-0.5 text-[12px] text-muted-foreground">
              Anything relevant that isn't on Canvas. PDFs and Office
              documents are read and indexed like the rest of the subject —
              other formats are kept, but not searchable.
            </p>
          </div>
          <Button size="sm" onClick={choose} disabled={busy}>
            {busy ? (
              <CircleNotch className="animate-spin" aria-hidden />
            ) : (
              <UploadSimple aria-hidden />
            )}
            {busy ? "Adding…" : "Add files"}
          </Button>
        </header>

        {problems.length > 0 && (
          <Alert variant="warning" className="mb-3 w-auto px-2.5 py-2">
            <Warning aria-hidden />
            <AlertDescription className="text-[11px] leading-snug">
              {problems.map((p) => (
                <span key={p} className="block">
                  {p}
                </span>
              ))}
            </AlertDescription>
          </Alert>
        )}

        {loading && uploads.length === 0 && !busy ? (
          <SkeletonRows count={4} />
        ) : empty ? (
          <EmptyState
            icon={<UploadSimple size={24} className="text-muted-foreground/40" aria-hidden />}
            title="Drop files here"
            body="PDF, Word, PowerPoint and Excel are parsed and become searchable."
          >
            <Button variant="outline" size="sm" onClick={choose}>
              Choose files
            </Button>
          </EmptyState>
        ) : (
          <ListCard>
            {importing.map((name) => (
              <ImportingRow key={`importing:${name}`} name={name} />
            ))}
            {uploads.map((f) => (
              <UploadRow
                key={f.id}
                file={f}
                status={liveStatuses[f.relative_path]}
                onDelete={() => setPendingDelete(f)}
              />
            ))}
          </ListCard>
        )}
      </SubjectPage>

      <ConfirmDialog
        open={!!pendingDelete}
        onCancel={() => setPendingDelete(null)}
        title="Remove this file?"
        description={
          pendingDelete
            ? `“${pendingDelete.filename}” is deleted from your library, along with what was read out of it. Nothing on Canvas changes, and your own copy of the file is untouched.`
            : ""
        }
        confirmLabel="Remove"
        busyLabel="Removing…"
        busy={deleting}
        onConfirm={async () => {
          if (!pendingDelete) return;
          setDeleting(true);
          try {
            await removeUpload(pendingDelete);
            setPendingDelete(null);
          } catch (e) {
            report(String(e));
            setPendingDelete(null);
          } finally {
            setDeleting(false);
          }
        }}
      />
    </>
  );
}

/** With no list for the Files tab's drop overlay to sit over, the drop target
 *  is drawn in the layout. "Drop files here" is true of the whole tab. */
/** A file mid-copy; same geometry as a real row so the list doesn't jump. */
function ImportingRow({ name }: { name: string }) {
  return (
    <div className="flex items-center gap-3 px-3 py-2">
      <CircleNotch size={14} className="shrink-0 animate-spin opacity-60" aria-hidden />
      <span className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">
        {name}
      </span>
      <span className="shrink-0 text-[11px] text-muted-foreground">Adding…</span>
    </div>
  );
}

function UploadRow({
  file,
  status,
  onDelete,
}: {
  file: DbFile;
  status: string | undefined;
  onDelete: () => void;
}) {
  const Icon = fileIconFor(file.filename);
  // Live status, else the stored column. Only PDF-backed files get a word.
  const label = useMemo(() => {
    if (!isPdfBacked(file.filename)) return "";
    switch (status ?? file.parse_status) {
      case "error": return "failed";
      case "queued":
      case "running": return "reading";
      case "fast":
      case "quality": return "parsed";
      default: return "";
    }
  }, [file.filename, file.parse_status, status]);

  return (
    <div className="group flex items-center gap-3 px-3 py-2 transition-colors hover:bg-surface">
      <button
        onClick={() => openFileSmart(file)}
        className="flex min-w-0 flex-1 items-center gap-3 text-left"
      >
        <Icon size={14} className="shrink-0 opacity-60" aria-hidden />
        <span className="flex-1 truncate text-[12px] text-foreground">
          {file.filename}
        </span>
        <span
          className={cn(
            "w-14 shrink-0 text-right text-[10px] uppercase tracking-wide",
            status === "error"
              ? "text-destructive"
              : label === "reading"
                ? "text-muted-foreground"
                : "text-success",
          )}
        >
          {label}
        </span>
        <span className="w-17 shrink-0 text-right text-[11px] text-muted-foreground tabular-nums">
          {fmtSize(file.size_bytes)}
        </span>
      </button>

      <span className="flex w-3 shrink-0 items-center justify-center">
        <button
          onClick={onDelete}
          aria-label={`Remove ${file.filename}`}
          title="Remove from this subject"
          className="text-muted-foreground opacity-0 transition-[color,opacity] group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100"
        >
          <Trash size={12} aria-hidden />
        </button>
      </span>
    </div>
  );
}
