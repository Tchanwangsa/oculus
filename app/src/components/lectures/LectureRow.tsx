import { memo } from "react";
import { CheckCircle, CircleNotch, Clock, DownloadSimple, Trash, X } from "@phosphor-icons/react";
import { cancelLectureDownload, dlKey, isDownloading, useLectureDownloads } from "@/stores/lectureDownloadStore";
import { fmtDurationSecs, fmtLectureDate, lecturePagePath, progressLabel } from "@/lib/lectures";
import type { Lecture } from "@/lib/db";
import { cn } from "@/lib/utils";

/**
 * A control inside the row's own `button`: a `div` with a button role, because
 * a nested `<button>` is invalid HTML and WebKit swallows its clicks. Stops
 * propagation so it never also opens the lecture.
 */
function RowAction({
  label,
  onTrigger,
  className,
  children,
}: {
  label: string;
  onTrigger: () => void;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      data-tab-skip
      aria-label={label}
      title={label}
      onClick={(e) => {
        e.stopPropagation();
        onTrigger();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          onTrigger();
        }
      }}
      className={cn(
        "-m-1 p-1 rounded text-muted-foreground/50 transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
        className,
      )}
    >
      {children}
    </div>
  );
}

/** Download ticks belong to one row, even when another subject is downloading. */
export const LectureRow = memo(function LectureRow({
  lecture: lec,
  active,
  onSelect,
  onDownload,
  onDelete,
}: {
  lecture: Lecture;
  active: boolean;
  onSelect: (lecture: Lecture) => void;
  onDownload: (lecture: Lecture) => void;
  onDelete: (lecture: Lecture) => void;
}) {
  const prog = useLectureDownloads((s) => s.progress[dlKey(lec.id)]);
  const downloading = useLectureDownloads((s) => isDownloading(s, lec.id));
  const isDown = !lec.video_path && downloading;
  const pl = progressLabel(lec);
  return (
    <button
      /* ⌘-click opens the lecture as a page of its own. */
      data-tab-href={lecturePagePath(lec)}
      onClick={() => onSelect(lec)}
      className={cn(
        "w-full text-left px-3 py-2.5 flex gap-3 items-center hover:bg-surface transition-colors",
        active && "bg-surface-raised",
      )}
    >
      <div className="shrink-0">
        {lec.completed ? (
          <CheckCircle size={14} className="text-success" />
        ) : (
          <div
            className={cn(
              "w-3 h-3 rounded-full border-2",
              active ? "border-brand" : "border-muted-foreground/40",
            )}
          />
        )}
      </div>
      <div className="min-w-0 flex-1">
        <p className="text-[12px] font-medium text-foreground truncate leading-tight">
          {lec.title}
        </p>
      </div>
      <span className="shrink-0 text-[11px] text-muted-foreground">
        {fmtLectureDate(lec.date)}
      </span>
      <span className="shrink-0 text-[11px] text-muted-foreground flex items-center gap-1 w-18 whitespace-nowrap">
        <Clock size={10} />
        {fmtDurationSecs(lec.duration_seconds)}
      </span>
      <span className={cn("shrink-0 text-[11px] w-20 text-right", pl.color)}>
        {pl.text}
      </span>
      {/* Downloaded ✓ + delete · downloading NN% + cancel ·
          otherwise a download trigger. */}
      <span className="shrink-0 w-16 flex items-center justify-end gap-1.5">
        {lec.video_path ? (
          <>
            <CheckCircle size={11} className="text-success" />
            <RowAction
              label="Delete download"
              onTrigger={() => onDelete(lec)}
              className="hover:text-destructive"
            >
              <Trash size={11} />
            </RowAction>
          </>
        ) : isDown ? (
          <>
            {prog == null || prog.phase === "trimming" ? (
              <CircleNotch size={11} className="animate-spin text-brand" />
            ) : (
              <span className="text-[10px] tabular-nums text-brand">
                {prog.percent}%
              </span>
            )}
            {/* Trimming has no transfer left to cancel. */}
            {prog?.phase !== "trimming" && (
              <RowAction
                label="Cancel download"
                onTrigger={() => cancelLectureDownload(lec.id)}
                className="hover:text-destructive"
              >
                <X size={11} />
              </RowAction>
            )}
          </>
        ) : (
          <RowAction
            label="Download video"
            onTrigger={() => onDownload(lec)}
            className="hover:text-foreground"
          >
            <DownloadSimple size={11} />
          </RowAction>
        )}
      </span>
    </button>
  );
});
