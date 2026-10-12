import { useEffect, useMemo, useState } from "react";
import { ClosedCaptioning, FilmStrip } from "@phosphor-icons/react";
import { SubjectLoading, SubjectPage, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { DownloadRow } from "@/components/files/DownloadRow";
import { ListCard } from "@/components/ui/layout/PageParts";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { TRANSCRIBED_EVENT, type TranscribedDetail } from "@/hooks/lectures/useTranscription";
import { FILE_SCRAPED_EVENT, type FileScraped } from "@/lib/pipeline/syncRunner";
import { courseFileHasContent } from "@/lib/files/courseFiles";
import { isVideoFile } from "@/lib/files/fileTypes";
import { useSubject } from "@/layouts/SubjectLayout";
import type { DbFile } from "@/lib/db";

/**
 * The videos Oculus fetched for the subject — Canvas module videos and Canvas
 * page embeds, both `file` rows. The student's own videos stay in Uploads, and
 * a video not yet downloaded shows only on its module.
 */
export default function SubjectMediaPage() {
  const subject = useSubject();
  const { byCategory, loading, reload } = useSubjectFiles(subject.id);
  const videos = useMemo(
    () => byCategory.file.filter((f) => isVideoFile(f.filename)),
    [byCategory.file],
  );

  // A download from the Modules page lands through the same `scrape-file`
  // handler as a sync's files.
  useWindowEvent(FILE_SCRAPED_EVENT, (e) => {
    if ((e as CustomEvent<FileScraped>).detail.subject_id === subject.id) reload();
  });

  if (loading && videos.length === 0) {
    return <SubjectLoading count={4} />;
  }

  if (videos.length === 0) {
    return (
      <SubjectEmpty
        icon={<FilmStrip size={24} className="text-muted-foreground/40" />}
        title="No videos yet."
      >
        <p className="text-xs text-muted-foreground/70 max-w-sm text-center">
          Module videos and videos embedded in Canvas pages appear here once downloaded.
        </p>
      </SubjectEmpty>
    );
  }

  return (
    <SubjectPage>
      <header className="mb-4">
        <h2 className="text-[13px] font-medium text-foreground">Videos</h2>
        <p className="mt-0.5 text-[12px] text-muted-foreground">
          Downloaded from the subject's modules and Canvas pages. Your own
          videos are in Uploads.
        </p>
      </header>
      <ListCard>
        {videos.map((f) => (
          <DownloadRow key={f.id} file={f} mark={<CaptionsMark file={f} />} />
        ))}
      </ListCard>
    </SubjectPage>
  );
}

/**
 * A captions icon when the video's sibling `.vtt` has content. The column is
 * always there and stays empty until the probe answers, so nothing shifts.
 */
function CaptionsMark({ file }: { file: DbFile }) {
  const rel = file.relative_path;
  const [hasCaptions, setHasCaptions] = useState(false);
  const [probe, setProbe] = useState(0);

  useEffect(() => {
    let stale = false;
    courseFileHasContent(`${rel}.vtt`)
      .then((has) => !stale && setHasCaptions(has))
      .catch(() => !stale && setHasCaptions(false));
    return () => {
      stale = true;
    };
  }, [rel, probe]);

  // Transcribe writes the VTT while this list may be mounted behind the player.
  useWindowEvent(TRANSCRIBED_EVENT, (e) => {
    if ((e as CustomEvent<TranscribedDetail>).detail.path === rel) setProbe((p) => p + 1);
  });

  return (
    <span className="shrink-0 w-4 flex items-center justify-center text-muted-foreground">
      {hasCaptions && (
        <span title="Has captions" aria-label="Has captions" className="flex">
          <ClosedCaptioning size={14} />
        </span>
      )}
    </span>
  );
}
