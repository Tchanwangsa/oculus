import { FileText, RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";

/** "queued" | "running" | "done" | "error" | undefined */
export type ParseBadgeStatus = "queued" | "running" | "done" | "error" | undefined;

// ── Extension badge ───────────────────────────────────────────────────────────

const EXT_STYLES: Record<string, string> = {
  pdf:  "bg-red-500/15 text-red-600 dark:text-red-400",
  docx: "bg-blue-500/15 text-blue-600 dark:text-blue-400",
  doc:  "bg-blue-500/15 text-blue-600 dark:text-blue-400",
  xlsx: "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400",
  xls:  "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400",
  pptx: "bg-orange-500/15 text-orange-600 dark:text-orange-400",
  ppt:  "bg-orange-500/15 text-orange-600 dark:text-orange-400",
  zip:  "bg-muted text-muted-foreground",
  mp4:  "bg-purple-500/15 text-purple-600 dark:text-purple-400",
  mp3:  "bg-purple-500/15 text-purple-600 dark:text-purple-400",
};

function ExtBadge({ filename }: { filename: string }) {
  const ext = filename.split(".").pop()?.toLowerCase();
  if (!ext) return null;
  const style = EXT_STYLES[ext] ?? "bg-muted text-muted-foreground";
  return (
    <span className={cn("shrink-0 text-[9px] font-semibold uppercase px-1 py-px rounded tracking-wide", style)}>
      {ext}
    </span>
  );
}

// ── Parse dot ─────────────────────────────────────────────────────────────────

function ParseDot({ status }: { status: ParseBadgeStatus }) {
  if (!status) return null;
  if (status === "queued" || status === "running") {
    return <span className="shrink-0 w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" title={status} />;
  }
  if (status === "done") {
    return <span className="shrink-0 w-1.5 h-1.5 rounded-full bg-emerald-500" title="Markdown ready" />;
  }
  if (status === "error") {
    return <span className="shrink-0 w-1.5 h-1.5 rounded-full bg-destructive" title="Parse failed" />;
  }
  return null;
}

// ── FileRow ───────────────────────────────────────────────────────────────────

interface FileRowProps {
  icon: typeof FileText;
  label: string;
  size: string;
  active: boolean;
  onClick: () => void;
  dimmed?: boolean;
  rightIcon?: typeof FileText;
  onRescrape?: () => void;
  isRescaping?: boolean;
  parseStatus?: ParseBadgeStatus;
  /** If set, shows a coloured extension pill (e.g. for Downloads section). */
  showExtBadge?: boolean;
}

export function FileRow({
  icon: Icon,
  label,
  size,
  active,
  onClick,
  dimmed = false,
  rightIcon: RightIcon,
  onRescrape,
  isRescaping = false,
  parseStatus,
  showExtBadge = false,
}: FileRowProps) {
  return (
    <div
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === "Enter" && onClick()}
      className={cn(
        "w-full text-left px-3 py-2 mx-1 rounded-lg flex items-center gap-2 transition-colors cursor-pointer group",
        active
          ? "bg-surface-raised text-foreground"
          : "text-muted-foreground hover:bg-surface hover:text-foreground",
        dimmed && !active && "opacity-70",
      )}
      style={{ width: "calc(100% - 8px)" }}
    >
      {showExtBadge ? <ExtBadge filename={label} /> : <Icon size={13} className="shrink-0" />}
      <span className="text-xs flex-1 truncate">{label}</span>
      <ParseDot status={parseStatus} />
      <span className="text-[10px] text-muted-foreground/70">{size}</span>
      {onRescrape ? (
        <button
          title={isRescaping ? "Re-downloading..." : "Re-download file"}
          disabled={isRescaping}
          onClick={(e) => { e.stopPropagation(); onRescrape(); }}
          className={cn(
            "shrink-0 p-0.5 rounded transition-opacity",
            isRescaping
              ? "opacity-60"
              : "opacity-0 group-hover:opacity-70 hover:!opacity-100 hover:text-foreground",
          )}
        >
          <RefreshCw size={10} className={cn(isRescaping && "animate-spin")} />
        </button>
      ) : RightIcon ? (
        <RightIcon size={10} className="shrink-0 opacity-50" />
      ) : null}
    </div>
  );
}
