import { cn } from "@/lib/utils";
import type { Subject } from "@/lib/db";

const PALETTE = [
  "#8b93e8",
  "#6ba5d7",
  "#a78bfa",
  "#d79b6b",
  "#7bbf8e",
  "#d78bb0",
  "#c9b26b",
];

export function courseColor(code: string): string {
  let h = 0;
  for (let i = 0; i < code.length; i++) h = code.charCodeAt(i) + ((h << 5) - h);
  return PALETTE[Math.abs(h) % PALETTE.length];
}

interface CourseRowProps {
  subject: Subject;
  selected: boolean;
  color: string;
  onClick: () => void;
  dimmed?: boolean;
}

export function CourseRow({
  subject,
  selected,
  color,
  onClick,
  dimmed = false,
}: CourseRowProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "w-full text-left px-3 py-2.5 mx-1 rounded-lg flex items-center gap-3 transition-colors",
        selected
          ? "bg-surface-raised text-foreground"
          : "text-muted-foreground hover:bg-surface hover:text-foreground",
        dimmed && !selected && "opacity-60",
      )}
      style={{ width: "calc(100% - 8px)" }}
    >
      <div
        className="w-2 h-2 rounded-full shrink-0"
        style={{ backgroundColor: color }}
      />
      <div className="min-w-0">
        <p className="text-xs font-semibold truncate">{subject.code}</p>
        <p className="text-[11px] text-muted-foreground truncate">
          {subject.name}
        </p>
      </div>
    </button>
  );
}
