import { useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { Paperclip } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { SubjectLoading, SubjectPage, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useReconcileParseStatus } from "@/hooks/sync/useReconcileParseStatus";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { FILE_SCRAPED_EVENT, type FileScraped } from "@/lib/pipeline/syncRunner";
import { useSubject } from "@/layouts/SubjectLayout";
import { DownloadRow } from "@/components/files/DownloadRow";
import { isVideoFile } from "@/lib/files/fileTypes";
import { ListCard } from "@/components/ui/layout/PageParts";

/**
 * Every file downloaded from Canvas, flat, except videos, which Media lists.
 * PDFs open in the side panel; other types hand off to the system viewer.
 */
export default function SubjectDownloadsPage() {
  const subject = useSubject();
  const navigate = useNavigate();
  const { byCategory, loading, reload } = useSubjectFiles(subject.id);
  const downloads = useMemo(
    () => byCategory.file.filter((f) => !isVideoFile(f.filename)),
    [byCategory.file],
  );

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
    // Every download so far is a video: point at Media, not at a sync.
    const onlyVideos = byCategory.file.length > 0;
    return (
      <SubjectEmpty
        icon={<Paperclip size={24} className="text-muted-foreground/40" />}
        title={onlyVideos ? "No files downloaded besides videos." : "No files downloaded yet."}
      >
        <Button
          variant="link"
          className="h-auto p-0 text-xs font-normal"
          onClick={() => navigate(onlyVideos ? "../media" : "/sync")}
        >
          {onlyVideos ? "Open Media →" : "Run a sync →"}
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
