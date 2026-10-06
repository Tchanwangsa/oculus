import { useNavigate } from "react-router-dom";
import { Paperclip } from "@phosphor-icons/react";
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
 * Every file downloaded from Canvas, flat. PDFs open in the side panel; other
 * types hand off to the system viewer.
 */
export default function SubjectDownloadsPage() {
  const subject = useSubject();
  const navigate = useNavigate();
  const { byCategory, loading, reload } = useSubjectFiles(subject.id);
  const downloads = byCategory.file;

  useReconcileParseStatus(downloads);

  // The row is written by the app-level `scrape-file` handler in
  // `useBackendEvents`, which raises FILE_SCRAPED_EVENT once it has.
  useWindowEvent(FILE_SCRAPED_EVENT, (e) => {
    if ((e as CustomEvent<FileScraped>).detail.subject_id === subject.id) reload();
  });

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
      <ListCard>
        {downloads.map((f) => (
          <DownloadRow key={f.id} file={f} />
        ))}
      </ListCard>
    </SubjectPage>
  );
}

function DownloadRow({ file }: { file: DbFile }) {
  const Icon = fileIconFor(file.filename);

  return (
    <div className="flex items-center gap-3 px-3 py-2 hover:bg-surface transition-colors">
      <button
        data-tab-href={filePageHref(file) ?? undefined}
        onClick={() => openFileSmart(file)}
        className="flex items-center gap-3 flex-1 min-w-0 text-left"
      >
        <Icon size={14} className="shrink-0 opacity-60" />
        <span className="text-[12px] text-foreground truncate flex-1">
          {file.filename}
        </span>
        {/* Fixed-width columns so rows line up. */}
        <span className="shrink-0 w-17 text-right text-[11px] text-muted-foreground tabular-nums">
          {fmtSize(file.size_bytes)}
        </span>
        <span className="shrink-0 w-13 flex items-center justify-end">
          <FileRecency file={file} />
        </span>
      </button>

      {/* Outside the row's button: a failed parse's icon is its own button.
          A blank column means "no parse", never "all is well". */}
      <span className="shrink-0 w-4 flex items-center justify-center">
        <ParseStateBadge file={file} />
      </span>
    </div>
  );
}
