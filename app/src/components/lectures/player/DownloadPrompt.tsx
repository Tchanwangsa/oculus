import { CircleNotch, DownloadSimple } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import type { Lecture } from "@/lib/db";
import { fmtDurationSecs, fmtLectureDate } from "@/lib/lectures";

interface DownloadPromptProps {
  lecture: Lecture;
  downloading: boolean;
  progress: { percent: number; phase: string } | null;
  onDownload: () => void;
}

/** What the frame shows while the video is not on disk. */
export function DownloadPrompt({ lecture, downloading, progress, onDownload }: DownloadPromptProps) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center gap-4 text-white/60 p-6">
      <p className="text-sm font-medium text-white">{lecture.title}</p>
      <p className="text-xs">
        {fmtLectureDate(lecture.date)} · {fmtDurationSecs(lecture.duration_seconds)}
      </p>
      {downloading ? (
        <div className="flex items-center gap-2 text-sm">
          <CircleNotch size={16} className="animate-spin" />
          <span>
            {progress?.phase === "trimming"
              ? "Trimming…"
              : `Downloading… ${progress?.percent ?? 0}%`}
          </span>
        </div>
      ) : (
        <Button
          size="sm"
          className="gap-2 bg-white/10 hover:bg-white/20 text-white border-white/20"
          variant="outline"
          onClick={onDownload}
        >
          <DownloadSimple size={14} /> Download video
        </Button>
      )}
    </div>
  );
}
