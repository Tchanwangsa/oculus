import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { getSyncRunFiles, type SyncRunFile, type SyncRunSummary } from "@/lib/db";
import { displayCode } from "@/lib/format/format";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { INLINE_FILE_LIMIT } from "@/components/sync/history/constants";
import { FileLine } from "@/components/sync/history/FileLine";
import { RunFilesDialog } from "@/components/sync/history/RunFilesDialog";

/** Codes the run targeted: the stored list, else (older runs) the ledger's. */
function runSubjectCodes(run: SyncRunSummary, files: SyncRunFile[]): string[] {
  if (run.subject_codes) {
    try {
      const parsed = JSON.parse(run.subject_codes);
      if (Array.isArray(parsed) && parsed.length > 0) return parsed;
    } catch {
      /* fall through to the ledger */
    }
  }
  return [...new Set(files.map((f) => f.subject_code).filter((c): c is string => !!c))];
}

function SubjectChips({ codes }: { codes: string[] }) {
  if (codes.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1.5 py-1.5">
      {codes.map((code) => (
        <span
          key={code}
          className="inline-flex items-center gap-1.5 rounded-md border border-border-subtle bg-surface px-2 py-1 text-[11px] text-foreground"
        >
          <SubjectIcon code={code} size={11} />
          {displayCode(code)}
        </span>
      ))}
    </div>
  );
}

export function RunDetail({ run }: { run: SyncRunSummary }) {
  const [files, setFiles] = useState<SyncRunFile[] | null>(null);
  const [modalOpen, setModalOpen] = useState(false);

  useEffect(() => {
    let alive = true;
    getSyncRunFiles(run.id)
      .then((rows) => alive && setFiles(rows))
      .catch(() => alive && setFiles([]));
    return () => {
      alive = false;
    };
    // Refetch while the run is live so the list grows with the sync.
  }, [run.id, run.file_count, run.status]);

  if (files === null) {
    return <div className="px-12 py-3 text-xs text-muted-foreground">Loading…</div>;
  }

  const changed = files.filter((f) => f.action !== "unchanged");
  const shown = changed.slice(0, INLINE_FILE_LIMIT);
  const skipped = run.unchanged_count;

  return (
    <div className="px-12 pb-3 pt-1">
      {run.error && (
        <p data-selectable className="text-xs text-destructive py-1.5">{run.error}</p>
      )}

      <SubjectChips codes={runSubjectCodes(run, files)} />

      {files.length === 0 ? (
        <p className="text-xs text-muted-foreground py-1.5">
          {run.status === "running"
            ? "Nothing touched yet…"
            : "No per-file records for this run."}
        </p>
      ) : (
        <>
          {shown.length > 0 ? (
            <div className="divide-y divide-border-subtle">
              {shown.map((f) => (
                <FileLine key={f.id} file={f} />
              ))}
            </div>
          ) : (
            <p className="text-xs text-muted-foreground py-1.5">
              Nothing new — every file was already up to date.
            </p>
          )}

          <div className="flex items-center gap-3 pt-2">
            {changed.length > shown.length && (
              <span className="text-[11px] text-muted-foreground">
                +{changed.length - shown.length} more changed
              </span>
            )}
            {skipped > 0 && shown.length > 0 && (
              <span className="text-[11px] text-muted-foreground">
                {skipped} unchanged file{skipped === 1 ? "" : "s"} skipped
              </span>
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setModalOpen(true)}
              className="h-6 px-2 text-[11px] text-muted-foreground hover:text-foreground -ml-2"
            >
              View all {files.length} files
            </Button>
          </div>

          <RunFilesDialog run={run} files={files} open={modalOpen} onOpenChange={setModalOpen} />
        </>
      )}
    </div>
  );
}
