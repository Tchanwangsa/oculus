import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ArrowsClockwise, CircleNotch, Paperclip } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useParseStore } from "@/stores/parseStore";
import { useSubject } from "@/layouts/SubjectLayout";
import { openFileSmart } from "@/lib/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { fileIconFor, isPdfBacked } from "@/lib/fileTypes";
import { fmtSize } from "@/lib/format";
import {
  upsertFile,
  setParseStatusByPath,
  type DbFile,
} from "@/lib/db";

/**
 * Every file downloaded from Canvas, flat. PDFs peek in-app; other types hand
 * off to the system viewer.
 */
export default function SubjectDownloadsPage() {
  const subject = useSubject();
  const navigate = useNavigate();
  const { byCategory, loading, reload } = useSubjectFiles(subject.id);
  const downloads = byCategory.file;

  const [rescraping, setRescraping] = useState<Set<number>>(new Set());
  const [rescrapeError, setRescrapeError] = useState<string | null>(null);

  const liveStatuses = useParseStore((s) => s.statuses);
  const mergeParseStatuses = useParseStore((s) => s.merge);

  // Reconcile parse status from disk: PDF-backed files with parse output get a
  // badge even if parsed before status tracking existed.
  useEffect(() => {
    const pdfPaths = downloads
      .filter((f) => isPdfBacked(f.filename))
      .map((f) => f.relative_path);
    if (pdfPaths.length === 0) return;
    invoke<Array<[string, string]>>("scan_parsed_files", { relativePaths: pdfPaths })
      .then((entries) => {
        if (entries.length === 0) return;
        mergeParseStatuses(Object.fromEntries(entries));
        setParseStatusByPath(entries).catch(() => {});
      })
      .catch(() => {});
  }, [downloads, mergeParseStatuses]);

  // Rescrape completions arrive as scrape-file events.
  useEffect(() => {
    const unsub = listen<{
      subject_id: number;
      relative_path: string;
      size_bytes: number;
      category: string | null;
      canvas_id: number | null;
    }>("scrape-file", async (e) => {
      const { subject_id, relative_path, size_bytes, category, canvas_id } = e.payload;
      const filename = relative_path.split("/").pop() ?? relative_path;
      const ext = filename.includes(".") ? filename.split(".").pop()! : "md";
      try {
        await upsertFile(subject_id, filename, relative_path, ext, size_bytes, category ?? undefined, canvas_id ?? undefined);
      } catch { /* ignore */ }
      if (canvas_id != null) {
        setRescraping((prev) => {
          const s = new Set(prev);
          s.delete(canvas_id);
          return s;
        });
      }
      if (subject_id === subject.id) reload();
    });
    return () => {
      unsub.then((f) => f());
    };
  }, [subject.id, reload]);

  const rescrape = async (file: DbFile) => {
    if (!file.canvas_id) return;
    setRescrapeError(null);
    setRescraping((prev) => new Set(prev).add(file.canvas_id!));
    try {
      await invoke("rescrape_file", {
        subjectId: file.subject_id,
        subjectCode: subject.code,
        canvasId: file.canvas_id,
      });
    } catch (err) {
      setRescraping((prev) => {
        const s = new Set(prev);
        s.delete(file.canvas_id!);
        return s;
      });
      setRescrapeError(String(err));
    }
  };

  if (loading && downloads.length === 0) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="mx-auto max-w-5xl px-6 py-6 space-y-2">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-8 w-full" />
          ))}
        </div>
      </div>
    );
  }

  if (downloads.length === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2">
        <Paperclip size={24} className="text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">No files downloaded yet.</p>
        <Button
          variant="link"
          className="h-auto p-0 text-xs font-normal"
          onClick={() => navigate("/sync")}
        >
          Run a sync →
        </Button>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-5xl px-6 py-5">
        {rescrapeError && (
          <Alert variant="destructive" className="mb-3 w-auto px-2.5 py-2">
            <AlertDescription className="text-[11px] leading-snug">
              {rescrapeError}
            </AlertDescription>
          </Alert>
        )}

        <div className="rounded-lg border border-border divide-y divide-border-subtle overflow-hidden">
          {downloads.map((f) => (
            <DownloadRow
              key={f.id}
              file={f}
              status={liveStatuses[f.relative_path]}
              rescraping={f.canvas_id != null && rescraping.has(f.canvas_id)}
              onRescrape={() => rescrape(f)}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function DownloadRow({
  file, status, rescraping, onRescrape,
}: {
  file: DbFile;
  status: string | undefined;
  rescraping: boolean;
  onRescrape: () => void;
}) {
  const Icon = fileIconFor(file.filename);

  return (
    <div className="group flex items-center gap-3 px-3 py-2 hover:bg-surface transition-colors">
      <button
        onClick={() => openFileSmart(file)}
        className="flex items-center gap-3 flex-1 min-w-0 text-left"
      >
        <Icon size={14} className="shrink-0 opacity-60" />
        <span className="text-[12px] text-foreground truncate flex-1">
          {file.filename}
        </span>
        {/* Fixed-width, right-aligned columns so every row lines up. */}
        <span
          className={cn(
            "shrink-0 w-12 text-right text-[10px] uppercase tracking-wide",
            status === "error" ? "text-destructive" : "text-success",
          )}
        >
          {status === "quality" ? "parsed" : status === "error" ? "failed" : ""}
        </span>
        <span className="shrink-0 w-17 text-right text-[11px] text-muted-foreground tabular-nums">
          {fmtSize(file.size_bytes)}
        </span>
        <span className="shrink-0 w-13 flex items-center justify-end">
          <FileRecency file={file} />
        </span>
      </button>

      {/* Slot is always reserved so the columns don't shift on rows without
          a Canvas id. */}
      <span className="shrink-0 w-3 flex items-center justify-center">
        {file.canvas_id != null && (
          <button
            onClick={onRescrape}
            disabled={rescraping}
            aria-label="Re-download from Canvas"
            title="Re-download from Canvas"
            className={cn(
              "text-muted-foreground hover:text-foreground transition-[color,opacity]",
              rescraping ? "opacity-100" : "opacity-0 group-hover:opacity-100",
            )}
          >
            {rescraping ? (
              <CircleNotch size={12} className="animate-spin" />
            ) : (
              <ArrowsClockwise size={12} />
            )}
          </button>
        )}
      </span>
    </div>
  );
}
