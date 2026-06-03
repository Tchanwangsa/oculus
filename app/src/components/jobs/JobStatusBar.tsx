import { useState, type ElementType } from "react";
import {
  ChevronUp,
  ChevronDown,
  Loader2,
  FileText,
  RefreshCw,
  Video,
  Download,
  Pause,
  AlertCircle,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useJobStore, type Job, type JobType } from "@/stores/jobStore";

// ── Config ────────────────────────────────────────────────────────────────────

const TYPE_ICON: Record<JobType, ElementType> = {
  pdf_parse: FileText,
  canvas_sync: RefreshCw,
  lecture_sync: Video,
  lecture_download: Download,
  transcript_download: FileText,
};

const TYPE_LABEL: Record<JobType, string> = {
  pdf_parse: "PDF parsing",
  canvas_sync: "Canvas sync",
  lecture_sync: "Lecture sync",
  lecture_download: "Lecture download",
  transcript_download: "Transcript download",
};

// ── Progress bar ──────────────────────────────────────────────────────────────

function ProgressBar({
  current,
  total,
  running,
  failed,
  className,
}: {
  current: number;
  total: number;
  running: boolean;
  failed: boolean;
  className?: string;
}) {
  const indeterminate = total === 0 && running;
  const pct = total > 0 ? Math.min((current / total) * 100, 100) : 0;

  return (
    <div className={cn("rounded-full bg-muted overflow-hidden", className)}>
      <div
        className={cn(
          "h-full rounded-full transition-all duration-300",
          failed ? "bg-destructive" : "bg-primary",
          indeterminate && "animate-pulse",
        )}
        style={{ width: indeterminate ? "100%" : `${pct}%` }}
      />
    </div>
  );
}

// ── Expanded job row ──────────────────────────────────────────────────────────

function JobRow({ job }: { job: Job }) {
  const Icon = TYPE_ICON[job.type];
  const isRunning = job.status === "running";
  const isFailed = job.status === "failed";
  const isPaused = job.status === "paused";
  const hasProgress = job.progress_total > 0;

  return (
    <div className="flex items-center gap-3 py-2.5 px-4">
      {/* Icon */}
      <div
        className={cn(
          "shrink-0",
          isRunning
            ? "text-primary"
            : isFailed
              ? "text-destructive"
              : "text-muted-foreground",
        )}
      >
        {isRunning ? (
          <Loader2 size={13} className="animate-spin" />
        ) : isFailed ? (
          <AlertCircle size={13} />
        ) : isPaused ? (
          <Pause size={13} />
        ) : (
          <Icon size={13} />
        )}
      </div>

      {/* Label + progress */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center justify-between gap-2 mb-1.5">
          <span className="text-xs font-medium text-foreground truncate">
            {TYPE_LABEL[job.type]}
            {job.label ? (
              <span className="text-muted-foreground font-normal">
                {" "}— {job.label}
              </span>
            ) : null}
          </span>
          {hasProgress && (
            <span className="text-[11px] text-muted-foreground shrink-0 tabular-nums">
              {job.progress_current}/{job.progress_total}
            </span>
          )}
        </div>
        <ProgressBar
          current={job.progress_current}
          total={job.progress_total}
          running={isRunning}
          failed={isFailed}
          className="h-1"
        />
        {job.error && (
          <p className="text-[11px] text-destructive mt-1 truncate">{job.error}</p>
        )}
      </div>

      {/* Status chip */}
      <span
        className={cn(
          "shrink-0 text-[10px] uppercase tracking-wider font-semibold w-14 text-right",
          isRunning
            ? "text-primary"
            : isFailed
              ? "text-destructive"
              : isPaused
                ? "text-amber-500"
                : "text-muted-foreground",
        )}
      >
        {job.status}
      </span>
    </div>
  );
}

// ── Status bar ────────────────────────────────────────────────────────────────

export function JobStatusBar() {
  const [expanded, setExpanded] = useState(false);
  const jobsRecord = useJobStore((s) => s.jobs);
  const allJobs = Object.values(jobsRecord);

  const visible = allJobs.filter(
    (j) => j.status === "running" || j.status === "queued" || j.status === "paused",
  );

  if (visible.length === 0) return null;

  // Primary job: prefer running over queued over paused
  const primary =
    visible.find((j) => j.status === "running") ??
    visible.find((j) => j.status === "queued") ??
    visible[0];

  const extra = visible.length - 1;
  const hasProgress = primary.progress_total > 0;

  return (
    <div className="border-t border-border bg-card shrink-0">
      {/* Expanded list */}
      {expanded && (
        <div className="max-h-[220px] overflow-y-auto divide-y divide-border">
          {visible.map((job) => (
            <JobRow key={job.id} job={job} />
          ))}
        </div>
      )}

      {/* Collapsed strip */}
      <button
        className="flex items-center gap-2.5 px-4 w-full h-9 hover:bg-muted/40 transition-colors"
        onClick={() => setExpanded((v) => !v)}
      >
        {/* Leading icon */}
        {primary.status === "running" ? (
          <Loader2 size={12} className="shrink-0 text-primary animate-spin" />
        ) : (
          <Pause size={12} className="shrink-0 text-muted-foreground" />
        )}

        {/* Job label */}
        <span className="text-xs font-medium text-foreground shrink-0 max-w-[140px] truncate">
          {TYPE_LABEL[primary.type]}
          {primary.label ? (
            <span className="text-muted-foreground font-normal"> — {primary.label}</span>
          ) : null}
        </span>

        {/* Progress bar */}
        <ProgressBar
          current={primary.progress_current}
          total={primary.progress_total}
          running={primary.status === "running"}
          failed={primary.status === "failed"}
          className="flex-1 h-1 mx-1"
        />

        {/* Count */}
        {hasProgress && (
          <span className="text-[11px] text-muted-foreground shrink-0 tabular-nums">
            {primary.progress_current}/{primary.progress_total}
          </span>
        )}

        {/* Extra count */}
        {extra > 0 && (
          <span className="text-[11px] text-muted-foreground/60 shrink-0">
            +{extra}
          </span>
        )}

        {/* Expand toggle */}
        <span className="shrink-0 text-muted-foreground ml-1">
          {expanded ? <ChevronDown size={12} /> : <ChevronUp size={12} />}
        </span>
      </button>
    </div>
  );
}
