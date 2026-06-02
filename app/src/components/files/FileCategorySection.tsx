import { ChevronDown, ChevronRight } from "lucide-react";
import { FileText } from "lucide-react";
import { FileRow } from "./FileRow";
import type { DbFile } from "@/lib/db";
import { fmtSize } from "@/lib/format";

interface FileCategorySectionProps {
  label: string;
  icon: typeof FileText;
  files: DbFile[];
  expanded: boolean;
  onToggle: () => void;
  activeFileId: number | null;
  onOpenFile: (f: DbFile) => void;
  dimmed?: boolean;
  rightIcon?: typeof FileText;
  onRescrape?: (f: DbFile) => void;
  rescraping?: Set<number>;
  labelFormatter?: (filename: string) => string;
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
}: FileCategorySectionProps) {
  if (files.length === 0) return null;

  return (
    <div className="mt-2">
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 px-4 py-1.5 text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors"
      >
        {expanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
        <Icon size={11} />
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
              f.canvas_id != null && onRescrape
                ? () => onRescrape(f)
                : undefined
            }
            isRescaping={
              f.canvas_id != null && rescraping
                ? rescraping.has(f.canvas_id)
                : false
            }
          />
        ))}
    </div>
  );
}
