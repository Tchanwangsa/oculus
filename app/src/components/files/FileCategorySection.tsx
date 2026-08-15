import {
  ChevronDownIcon,
  ChevronRightIcon,
  DocumentTextIcon,
} from "@heroicons/react/16/solid";
import { FileRow, type ParseBadgeStatus } from "./FileRow";
import type { DbFile } from "@/lib/db";
import { fmtSize } from "@/lib/format";

interface FileCategorySectionProps {
  label: string;
  icon: typeof DocumentTextIcon;
  files: DbFile[];
  expanded: boolean;
  onToggle: () => void;
  activeFileId: number | null;
  onOpenFile: (f: DbFile) => void;
  dimmed?: boolean;
  rightIcon?: typeof DocumentTextIcon;
  onRescrape?: (f: DbFile) => void;
  rescraping?: Set<number>;
  labelFormatter?: (filename: string) => string;
  /** Live overrides keyed by relative_path (from in-flight parse events). Falls back to DB f.parse_status. */
  liveStatuses?: Record<string, string>;
  /** Show coloured extension pill instead of category icon (e.g. Downloads). */
  showExtBadge?: boolean;
}

function badgeStatus(
  f: DbFile,
  liveStatuses: Record<string, string> | undefined,
): ParseBadgeStatus {
  if (!f.filename.toLowerCase().endsWith(".pdf")) return undefined;
  const s = liveStatuses?.[f.relative_path] ?? f.parse_status ?? undefined;
  if (s === "queued" || s === "running" || s === "fast") return "running"; // quality pending
  if (s === "quality" || s === "done") return "done";
  if (s && s.startsWith("error")) return "error";
  return undefined;
}

export function FileCategorySection({
  label,
  icon: Icon,
  files,
  expanded,
  onToggle,
  activeFileId,
  onOpenFile,
  dimmed = false,
  rightIcon,
  onRescrape,
  rescraping,
  labelFormatter,
  liveStatuses,
  showExtBadge = false,
}: FileCategorySectionProps) {
  if (files.length === 0) return null;

  return (
    <div className="mt-2">
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 px-4 py-1.5 text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors"
      >
        {expanded ? <ChevronDownIcon className="size-[11px]" /> : <ChevronRightIcon className="size-[11px]" />}
        <Icon className="size-[11px]" />
        {label} ({files.length})
      </button>
      {expanded &&
        files.map((f) => (
          <FileRow
            key={f.id}
            icon={Icon}
            label={labelFormatter ? labelFormatter(f.filename) : f.filename}
            size={fmtSize(f.size_bytes)}
            active={activeFileId === f.id}
            onClick={() => onOpenFile(f)}
            dimmed={dimmed}
            rightIcon={rightIcon}
            onRescrape={
              f.canvas_id != null && onRescrape ? () => onRescrape(f) : undefined
            }
            isRescaping={
              f.canvas_id != null && rescraping ? rescraping.has(f.canvas_id) : false
            }
            parseStatus={badgeStatus(f, liveStatuses)}
            showExtBadge={showExtBadge}
          />
        ))}
    </div>
  );
}
