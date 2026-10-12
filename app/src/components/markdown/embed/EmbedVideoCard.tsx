import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  ArrowClockwise,
  ArrowSquareOut,
  CircleNotch,
  ClosedCaptioning,
  DownloadSimple,
  Play,
  VideoCamera,
  X,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { EchoVideoPlaceholder } from "@/components/markdown/embed/EchoVideoPlaceholder";
import { useEmbedSubject } from "@/components/markdown/embed/EmbedSubject";
import { IconAction } from "@/components/markdown/embed/IconAction";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { openExternal } from "@/lib/browser";
import { isVideoFile } from "@/lib/files/fileTypes";
import { filePagePath, openFileSmart } from "@/lib/files/openFile";
import { attachmentSrc } from "@/lib/harness/attachments";
import { fmtDurationSecs } from "@/lib/lectures";
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

/**
 * An Echo360 video embedded in a Canvas page (`<echo-video>`, written by the
 * sync's HTML converter): poster, name and length, then Play once downloaded
 * or Download before. Needs the library file's subject
 * (`EmbedSubjectProvider`); without one it is only a link out. Built from
 * spans, since the tag can land inside a paragraph or a table cell.
 */
export function EmbedVideoCard({ linkId, src }: { linkId?: string; src?: string }) {
  const subject = useEmbedSubject();
  const embed = useMemo<EmbedRef | null>(
    () =>
      linkId && src && subject && subject !== "loading"
        ? { subjectId: subject.subjectId, subjectCode: subject.subjectCode, linkId, src }
        : null,
    [linkId, src, subject],
  );
  if (!src) return null;
  if (!linkId || subject === null) return <EchoVideoPlaceholder linkId={linkId} src={src} />;
  if (!embed) return <CardSkeleton />;
  return <ResolvedCard embed={embed} />;
}

function ResolvedCard({ embed }: { embed: EmbedRef }) {
  const info = useEmbeds((s) => s.infos[embed.linkId]);
  const download = useEmbeds((s) => s.downloads[embed.linkId]);

  useEffect(() => {
    void embedInfo(embed);
  }, [embed]);

  const md = tagSource(embed);
  const openInCanvas = (e: React.MouseEvent) => void openExternal(embed.src, e.metaKey);
  const canvasIcon = (
    <IconAction label="Open in Canvas" size="icon-sm" onClick={openInCanvas}>
      <ArrowSquareOut size={14} />
    </IconAction>
  );

  if (!info || info.status === "loading") return <CardSkeleton md={md} />;

  if (info.status === "error") {
    return (
      <CardShell
        md={md}
        poster={<Poster rel={null} />}
        title="Echo360 video"
        meta={<ErrorText>{info.error}</ErrorText>}
        actions={
          <>
            <Button variant="outline" onClick={() => void embedInfo(embed)}>
              <ArrowClockwise />
              Retry
            </Button>
            {canvasIcon}
          </>
        }
      />
    );
  }

  const row = info.row;
  return (
    <CardShell
      md={md}
      poster={<Poster key={row.posterRel} rel={row.posterRel} />}
      title={displayName(row.name)}
      titleHint={row.name}
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
      actions={
        <RowActions embed={embed} row={row} download={download} canvasIcon={canvasIcon} onCanvas={openInCanvas} />
      }
    />
  );
}

function RowActions({
  embed,
  row,
  download,
  canvasIcon,
  onCanvas,
}: {
  embed: EmbedRef;
  row: EmbedRow;
  download: EmbedDownload | undefined;
  canvasIcon: ReactNode;
  onCanvas: (e: React.MouseEvent) => void;
}) {
  const fileRel = row.fileRel;

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
      <Button variant="outline" onClick={onCanvas}>
        <ArrowSquareOut />
        Open in Canvas
      </Button>
    );
  }

  if (download?.status === "downloading") {
    return (
      <>
        <span
          className="flex h-8 min-w-10 items-center justify-end px-1 text-xs tabular-nums text-brand"
          title={download.muxing ? "Joining the video and audio" : undefined}
        >
          {download.muxing ? (
            "Joining…"
          ) : download.percent == null ? (
            <CircleNotch size={14} className="animate-spin" />
          ) : (
            `${download.percent}%`
          )}
        </span>
        <IconAction
          label="Cancel download"
          size="icon-sm"
          onClick={() => void cancelEmbedDownload(embed.linkId)}
        >
          <X size={14} />
        </IconAction>
      </>
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

const CARD = "my-3 flex max-w-xl select-none items-center gap-3 rounded-lg border border-border bg-surface p-2";

/** One row: a 16:9 thumbnail, two lines of text, the actions. The thumbnail
 *  sets the height, so the skeleton and every state are the same size. */
function CardShell({
  md,
  poster,
  title,
  titleHint,
  meta,
  actions,
}: {
  md: string;
  poster: ReactNode;
  title: string;
  titleHint?: string;
  meta: ReactNode;
  actions: ReactNode;
}) {
  return (
    <span data-md={md} className={CARD}>
      {poster}
      <span className="block min-w-0 flex-1">
        <span className="block truncate text-sm font-medium text-foreground" title={titleHint ?? title}>
          {title}
        </span>
        <span className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-muted-foreground">
          {meta}
        </span>
      </span>
      <span className="flex shrink-0 items-center gap-1">{actions}</span>
    </span>
  );
}

function CardSkeleton({ md }: { md?: string }) {
  return (
    <span data-md={md} aria-hidden className={CARD}>
      <span className="block aspect-video w-32 shrink-0 animate-pulse rounded-md bg-accent" />
      <span className="block min-w-0 flex-1">
        <span className="block h-3.5 w-48 max-w-full animate-pulse rounded bg-accent" />
        <span className="mt-2 block h-3 w-16 animate-pulse rounded bg-accent" />
      </span>
    </span>
  );
}

/** The poster Rust saved under the course's `images/`; a camera glyph without one. */
function Poster({ rel }: { rel: string | null }) {
  const dataDir = useDataDir();
  const [broken, setBroken] = useState(false);
  // Not until the data dir resolves: an empty `src` fires `error`.
  const src = rel && !broken ? attachmentSrc(dataDir, rel) : "";
  return (
    <span className="flex aspect-video w-32 shrink-0 items-center justify-center overflow-hidden rounded-md border border-border-subtle bg-surface-raised text-muted-foreground">
      {src ? (
        <img src={src} alt="" onError={() => setBroken(true)} className="block size-full object-cover" />
      ) : (
        <VideoCamera size={18} />
      )}
    </span>
  );
}

/** An error verbatim, selectable like every error's detail. */
function ErrorText({ children }: { children: ReactNode }) {
  return <span className="select-text break-words text-destructive">{children}</span>;
}

/** Echo360's media name without its video extension. */
function displayName(name: string): string {
  return (isVideoFile(name) ? name.replace(/\.[^.]+$/, "") : name) || "Echo360 video";
}

/** The tag this card stands for, so a copied selection keeps it (`lib/markdown/selection.ts`). */
function tagSource({ linkId, src }: EmbedRef): string {
  const attr = (v: string) => v.replace(/&/g, "&amp;").replace(/"/g, "&quot;");
  return `<echo-video link-id="${attr(linkId)}" src="${attr(src)}">\n</echo-video>`;
}
