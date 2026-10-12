import { useEffect, useMemo } from "react";
import { ArrowClockwise, ArrowSquareOut, ClosedCaptioning, DownloadSimple, Play } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { CardShell, CardSkeleton, ErrorText } from "@/components/markdown/embed/card/CardShell";
import { DownloadProgress, OpenInCanvasIcon } from "@/components/markdown/embed/card/CardActions";
import { Thumbnail } from "@/components/markdown/embed/card/Thumbnail";
import { tagSource, videoDisplayName } from "@/components/markdown/embed/card/tagSource";
import { useEmbedSubject } from "@/components/markdown/embed/EmbedSubject";
import { VideoLinkOut } from "@/components/markdown/embed/VideoLinkOut";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { openLink } from "@/lib/shell/tabRouters";
import { filePagePath, openFileSmart } from "@/lib/files/openFile";
import { attachmentSrc } from "@/lib/harness/attachments";
import { fmtDurationSecs } from "@/lib/lectures/format";
import {
  DOWNLOAD_OFF,
  cancelEmbedDownload,
  downloadEmbed,
  embedFile,
  embedInfo,
  useEmbeds,
  type EmbedDownload,
  type EmbedRef,
  type EmbedRow,
} from "@/stores/lectures/embedDownloadStore";

const FALLBACK = "Echo360 video";

/**
 * An Echo360 video embedded in a Canvas page (`<echo-video>`, written by the
 * sync's HTML converter): poster, name and length, then Play once downloaded
 * or Download before. Needs the library file's subject
 * (`EmbedSubjectProvider`); without one it is only a link out. `title` is the
 * Canvas iframe's, shown until Echo360's media name is known.
 */
export function EmbedVideoCard({ linkId, src, title }: { linkId?: string; src?: string; title?: string }) {
  const subject = useEmbedSubject();
  const embed = useMemo<EmbedRef | null>(
    () =>
      linkId && src && subject && subject !== "loading"
        ? { subjectId: subject.subjectId, subjectCode: subject.subjectCode, linkId, src }
        : null,
    [linkId, src, subject],
  );
  if (!src) return null;
  const name = title?.trim() || undefined;
  if (!linkId || subject === null) return <VideoLinkOut label={name ?? FALLBACK} href={src} />;
  const md = tagSource("echo-video", [["link-id", linkId], ["src", src], ["title", name]]);
  if (!embed) return <CardSkeleton md={md} title={name} />;
  return <ResolvedCard embed={embed} md={md} name={name} />;
}

function ResolvedCard({ embed, md, name }: { embed: EmbedRef; md: string; name?: string }) {
  const info = useEmbeds((s) => s.infos[embed.linkId]);
  const download = useEmbeds((s) => s.downloads[embed.linkId]);

  useEffect(() => {
    void embedInfo(embed);
  }, [embed]);

  if (!info || info.status === "loading") return <CardSkeleton md={md} title={name} />;

  if (info.status === "error") {
    return (
      <CardShell
        md={md}
        thumbnail={<Thumbnail />}
        title={name ?? FALLBACK}
        meta={<ErrorText>{info.error}</ErrorText>}
        actions={
          <>
            <Button variant="outline" onClick={() => void embedInfo(embed)}>
              <ArrowClockwise />
              Retry
            </Button>
            <OpenInCanvasIcon href={embed.src} />
          </>
        }
      />
    );
  }

  const row = info.row;
  return (
    <CardShell
      md={md}
      thumbnail={<Poster key={row.posterRel} rel={row.posterRel} />}
      title={videoDisplayName(row.name, name ?? FALLBACK)}
      titleHint={row.name || name}
      meta={
        <>
          {row.durationS != null && (
            <span className="tabular-nums">{fmtDurationSecs(Math.round(row.durationS))}</span>
          )}
          {row.hasCaptions && (
            <span title="Has captions" aria-label="Has captions" className="flex">
              <ClosedCaptioning size={14} />
            </span>
          )}
          {row.restrictDownload && !row.fileRel && <span>{DOWNLOAD_OFF}</span>}
          {download?.status === "error" && <ErrorText>{download.error}</ErrorText>}
        </>
      }
      actions={<RowActions embed={embed} row={row} download={download} />}
    />
  );
}

function RowActions({
  embed,
  row,
  download,
}: {
  embed: EmbedRef;
  row: EmbedRow;
  download: EmbedDownload | undefined;
}) {
  const fileRel = row.fileRel;
  const canvasIcon = <OpenInCanvasIcon href={embed.src} />;

  if (fileRel) {
    const play = async () => {
      const file = await embedFile(embed, fileRel);
      if (file) openFileSmart(file);
    };
    return (
      <>
        <Button data-tab-href={filePagePath(embed.subjectId, fileRel)} onClick={() => void play()}>
          <Play weight="fill" />
          Play
        </Button>
        {canvasIcon}
      </>
    );
  }

  if (row.restrictDownload) {
    return (
      <Button variant="outline" onClick={(e) => void openLink(embed.src, e.metaKey)}>
        <ArrowSquareOut />
        Open in Canvas
      </Button>
    );
  }

  if (download?.status === "downloading") {
    return (
      <DownloadProgress
        percent={download.percent}
        label={download.muxing ? "Joining…" : undefined}
        hint={download.muxing ? "Joining the video and audio" : undefined}
        onCancel={() => void cancelEmbedDownload(embed.linkId)}
      />
    );
  }

  const failed = download?.status === "error";
  return (
    <>
      <Button variant="outline" onClick={() => void downloadEmbed(embed)}>
        {failed ? <ArrowClockwise /> : <DownloadSimple />}
        {failed ? "Retry" : "Download"}
      </Button>
      {canvasIcon}
    </>
  );
}

/** The poster Rust saved under the course's `images/`; `attachmentSrc` stays
 *  empty until the data dir resolves. */
function Poster({ rel }: { rel: string | null }) {
  const dataDir = useDataDir();
  return <Thumbnail src={rel ? attachmentSrc(dataDir, rel) : null} />;
}
