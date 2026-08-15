import {
  ArrowPathIcon,
  ExclamationCircleIcon,
  CheckIcon,
} from "@heroicons/react/16/solid";
import { cn } from "@/lib/utils";
import { useJobStore, type Job, type JobType } from "@/stores/jobStore";

const TYPE_LABEL: Record<JobType, string> = {
  pdf_parse: "Parsing PDFs",
  canvas_sync: "Syncing Canvas",
  lecture_sync: "Syncing lectures",
  lecture_download: "Downloading lecture",
  transcript_download: "Downloading transcript",
};

function JobEntry({ job }: { job: Job }) {
  const failed = job.status === "failed";
  const done = job.status === "completed";
  const indeterminate = job.progress_total === 0 && job.status === "running";
  const pct =
    job.progress_total > 0
      ? Math.min((job.progress_current / job.progress_total) * 100, 100)
      : 0;

  return (
    <div className="px-2.5 py-2">
      <div className="flex items-center gap-2 mb-1.5">
        {done ? (
          <CheckIcon className="size-[11px] shrink-0 text-success" />
        ) : failed ? (
          <ExclamationCircleIcon className="size-[11px] shrink-0 text-destructive" />
        ) : (
          <ArrowPathIcon className="size-[11px] shrink-0 text-muted-foreground animate-spin" />
        )}
        <span className="text-xs text-foreground truncate flex-1 min-w-0">
          {TYPE_LABEL[job.type]}
        </span>
        {job.progress_total > 0 && (
          <span className="text-[11px] text-muted-foreground tabular-nums shrink-0">
            {job.progress_current}/{job.progress_total}
          </span>
        )}
      </div>
      {job.label && (
        <p className="text-[11px] text-muted-foreground truncate mb-1.5 pl-[19px]">
          {job.label}
        </p>
      )}
      <div className="h-[3px] rounded-full bg-surface-overlay overflow-hidden">
        <div
          className={cn(
            "h-full rounded-full transition-all duration-300",
            failed ? "bg-destructive" : done ? "bg-success" : "bg-primary",
            indeterminate && "animate-pulse"
          )}
          style={{ width: indeterminate || done ? "100%" : `${pct}%` }}
        />
      </div>
      {job.error && (
        <p className="text-[11px] text-destructive mt-1 truncate">{job.error}</p>
      )}
    </div>
  );
}

/**
 * Compact activity panel at the bottom of the sidebar. The single place
 * background work (sync, PDF parsing, downloads) is surfaced — no toasts,
 * no bottom bar.
 */
export function SidebarActivity({ collapsed }: { collapsed: boolean }) {
  const jobsRecord = useJobStore((s) => s.jobs);
  const jobs = Object.values(jobsRecord).filter(
    (j) => j.status !== "paused",
  );

  if (jobs.length === 0) return null;

  if (collapsed) {
    return (
      <div className="px-2 pb-2 flex justify-center relative group">
        <div className="w-7 h-7 rounded-md bg-sidebar-item-hover flex items-center justify-center">
          <ArrowPathIcon className="size-[13px] text-muted-foreground animate-spin" />
        </div>
        <div className="absolute left-full bottom-1 ml-3 px-2 py-1 rounded-md bg-foreground text-background text-xs font-medium whitespace-nowrap opacity-0 pointer-events-none group-hover:opacity-100 transition-opacity z-50 shadow-md">
          {jobs.map((j) => TYPE_LABEL[j.type]).join(" · ")}
        </div>
      </div>
    );
  }

  return (
    <div className="mx-2 mb-2 rounded-lg border border-sidebar-border bg-card divide-y divide-border-subtle overflow-hidden">
      {jobs.map((job) => (
        <JobEntry key={job.id} job={job} />
      ))}
    </div>
  );
}
