import { ArrowClockwise, DownloadSimple, Play } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { CardShell, CardSkeleton, ErrorText } from "@/components/markdown/embed/card/CardShell";
import { DownloadProgress, OpenInCanvasIcon } from "@/components/markdown/embed/card/CardActions";
import { Thumbnail } from "@/components/markdown/embed/card/Thumbnail";
import { tagSource, videoDisplayName } from "@/components/markdown/embed/card/tagSource";
import { useEmbedSubject } from "@/components/markdown/embed/EmbedSubject";
import { useLibraryVideo } from "@/components/markdown/embed/useLibraryVideo";
import { VideoLinkOut } from "@/components/markdown/embed/VideoLinkOut";
import { canvasFileUrl } from "@/lib/files/canvasLinks";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { fmtSize } from "@/lib/format/format";
import { cancelVideoDownload, downloadVideo, useVideoDownloads } from "@/stores/lectures/videoDownloadStore";

const FALLBACK = "Canvas video";

/**
 * A video uploaded to Canvas and embedded in a page (`<canvas-video>`, written
 * by the sync's HTML converter). A sync never fetches one — they run to
 * hundreds of megabytes — so the card offers Download, through the same
 * `videoDownloadStore` as a module video, then Play. Without the library
 * file's subject (`EmbedSubjectProvider`) it is only a link out.
 */
export function CanvasVideoCard({ fileId, title }: { fileId?: string; title?: string }) {
  const subject = useEmbedSubject();
  if (!fileId || !/^\d+$/.test(fileId)) return null;
  const name = videoDisplayName(title, FALLBACK);
  if (subject === null) return <VideoLinkOut label={name} href={canvasFileUrl(fileId)} />;
  const md = tagSource("canvas-video", [["file-id", fileId], ["title", title?.trim()]]);
  if (subject === "loading") return <CardSkeleton md={md} title={name} />;
  return (
    <ResolvedCard
      subject={{ id: subject.subjectId, code: subject.subjectCode }}
      canvasId={Number(fileId)}
      md={md}
      name={name}
      titleHint={title}
    />
  );
}

function ResolvedCard({
  subject,
  canvasId,
  md,
  name,
  titleHint,
}: {
  subject: { id: number; code: string };
  canvasId: number;
  md: string;
  name: string;
  titleHint?: string;
}) {
  const { file, recheck } = useLibraryVideo(subject.id, canvasId);
  const download = useVideoDownloads((s) => s.downloads[canvasId]);
  const canvasIcon = <OpenInCanvasIcon href={canvasFileUrl(String(canvasId), subject.id)} />;

  if (file === undefined) return <CardSkeleton md={md} title={name} />;

  const shell = (meta: React.ReactNode, actions: React.ReactNode) => (
    <CardShell
      md={md}
      thumbnail={<Thumbnail />}
      title={name}
      titleHint={titleHint}
      meta={meta}
      actions={actions}
    />
  );

  // The looked-up row, never the store's `done` one: a finished download's
  // row write looks it up again, and a row removed since must fall back to
  // Download even after this session downloaded it.
  if (file) {
    const play = async () => {
      const current = await recheck();
      if (current) openFileSmart(current);
    };
    return shell(
      file.size_bytes != null && <span className="tabular-nums">{fmtSize(file.size_bytes)}</span>,
      <>
        <Button data-tab-href={filePageHref(file) ?? undefined} onClick={() => void play()}>
          <Play weight="fill" />
          Play
        </Button>
        {canvasIcon}
      </>,
    );
  }

  if (download?.status === "downloading") {
    return shell(
      null,
      <DownloadProgress percent={download.percent} onCancel={() => void cancelVideoDownload(canvasId)} />,
    );
  }

  const failed = download?.status === "error" ? download.error : null;
  return shell(
    failed && <ErrorText>{failed}</ErrorText>,
    <>
      <Button variant="outline" onClick={() => void downloadVideo(subject, canvasId)}>
        {failed ? <ArrowClockwise /> : <DownloadSimple />}
        {failed ? "Retry" : "Download"}
      </Button>
      {canvasIcon}
    </>,
  );
}
