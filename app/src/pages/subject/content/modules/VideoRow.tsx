import { useEffect, useRef, type ReactNode } from "react";
import { ArrowClockwise, CircleNotch, DownloadSimple, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { openFileSmart } from "@/lib/files/openFile";
import {
  cancelVideoDownload,
  downloadVideo,
  type VideoDownload,
} from "@/stores/lectures/videoDownloadStore";
import type { SubjectRef } from "./types";

/**
 * A module video not yet in the library: clicking downloads it (or retries a
 * failure) and opens it once it lands, if this row is still on screen.
 */
export function VideoRow({
  canvasFileId, subject, download, rowClass, children,
}: {
  canvasFileId: number;
  subject: SubjectRef;
  download: VideoDownload | undefined;
  rowClass: string;
  children: ReactNode;
}) {
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const start = async () => {
    const file = await downloadVideo(subject, canvasFileId);
    if (file && mounted.current) openFileSmart(file);
  };

  if (download?.status === "downloading") {
    return (
      <div className={cn(rowClass, "text-foreground")}>
        {children}
        {download.percent == null ? (
          <CircleNotch size={11} className="shrink-0 animate-spin text-brand" />
        ) : (
          <span className="shrink-0 text-[11px] tabular-nums text-brand">
            {download.percent}%
          </span>
        )}
        <button
          aria-label="Cancel download"
          title="Cancel download"
          onClick={() => void cancelVideoDownload(canvasFileId)}
          className="-m-1 shrink-0 rounded p-1 text-muted-foreground/60 transition-colors hover:text-destructive"
        >
          <X size={11} />
        </button>
      </div>
    );
  }

  const failed = download?.status === "error" ? download.error : null;
  return (
    <button
      onClick={() => void start()}
      title={failed ? `Download failed: ${failed}` : "Download video"}
      className={cn(rowClass, "group text-foreground hover:bg-surface")}
    >
      {children}
      {failed ? (
        <span className="flex min-w-0 max-w-[50%] shrink items-center gap-1 text-[11px] text-destructive">
          <span className="truncate">{failed}</span>
          <ArrowClockwise size={11} className="shrink-0" />
        </span>
      ) : (
        <DownloadSimple
          size={12}
          className="shrink-0 text-muted-foreground/50 transition-colors group-hover:text-foreground"
        />
      )}
    </button>
  );
}
