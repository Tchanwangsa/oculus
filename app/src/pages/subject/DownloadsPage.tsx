import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { ArrowsClockwise, CircleNotch, Paperclip } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { SubjectLoading, SubjectPage, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useReconcileParseStatus } from "@/hooks/useReconcileParseStatus";
import { useWindowEvent } from "@/hooks/useEvents";
import { FILE_SCRAPED_EVENT, type FileScraped } from "@/lib/syncRunner";
import { useSubject } from "@/layouts/SubjectLayout";
import { filePageHref, openFileSmart } from "@/lib/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { ParseStateBadge } from "@/components/files/ParseState";
import { fileIconFor } from "@/lib/fileTypes";
import { fmtSize } from "@/lib/format";
import type { DbFile } from "@/lib/db";
import { ListCard } from "@/components/ui/PageParts";

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

  useReconcileParseStatus(downloads);

  // The row is written by the app-level `scrape-file` handler in
  // `useBackendEvents`, which raises FILE_SCRAPED_EVENT once it has; a
  // rescrape's completion only refreshes the list and clears its spinner.
  useWindowEvent(FILE_SCRAPED_EVENT, (e) => {
    const { subject_id, canvas_id } = (e as CustomEvent<FileScraped>).detail;
    if (canvas_id != null) {
      setRescraping((prev) => {
        const s = new Set(prev);
        s.delete(canvas_id);
        return s;
      });
    }
    if (subject_id === subject.id) reload();
  });

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
    return <SubjectLoading count={8} />;
  }

  if (downloads.length === 0) {
    return (
      <SubjectEmpty icon={<Paperclip size={24} className="text-muted-foreground/40" />} title="No files downloaded yet.">
        <Button
          variant="link"
          className="h-auto p-0 text-xs font-normal"
          onClick={() => navigate("/sync")}
        >
          Run a sync →
        </Button>
      </SubjectEmpty>
    );
  }

  return (
    <SubjectPage>
      {rescrapeError && (
        <Alert variant="destructive" className="mb-3 w-auto px-2.5 py-2">
          <AlertDescription className="text-[11px] leading-snug">
            {rescrapeError}
          </AlertDescription>
        </Alert>
      )}

      <ListCard>
        {downloads.map((f) => (
          <DownloadRow
            key={f.id}
            file={f}
            rescraping={f.canvas_id != null && rescraping.has(f.canvas_id)}
            onRescrape={() => rescrape(f)}
          />
        ))}
      </ListCard>
    </SubjectPage>
  );
}

function DownloadRow({
  file, rescraping, onRescrape,
}: {
  file: DbFile;
  rescraping: boolean;
  onRescrape: () => void;
}) {
  const Icon = fileIconFor(file.filename);

  return (
    <div className="group flex items-center gap-3 px-3 py-2 hover:bg-surface transition-colors">
      <button
        data-tab-href={filePageHref(file) ?? undefined}
        onClick={() => openFileSmart(file)}
        className="flex items-center gap-3 flex-1 min-w-0 text-left"
      >
        <Icon size={14} className="shrink-0 opacity-60" />
        <span className="text-[12px] text-foreground truncate flex-1">
          {file.filename}
        </span>
        {/* Fixed-width columns so rows line up. A blank parse column means
            "no parse", never "all is well". */}
        <span className="shrink-0 w-20 flex items-center justify-end">
          <ParseStateBadge file={file} />
        </span>
        <span className="shrink-0 w-17 text-right text-[11px] text-muted-foreground tabular-nums">
          {fmtSize(file.size_bytes)}
        </span>
        <span className="shrink-0 w-13 flex items-center justify-end">
          <FileRecency file={file} />
        </span>
      </button>

      {/* Always reserved so rows without a Canvas id keep the columns. */}
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
