import { BookOpen } from "lucide-react";
import { cn } from "@/lib/utils";
import type { Subject } from "@/lib/db";

interface SubjectRowProps {
  subject: Subject;
  checked: boolean;
  onToggle: () => void;
  dimmed?: boolean;
}

export function SubjectRow({
  subject,
  checked,
  onToggle,
  dimmed = false,
}: SubjectRowProps) {
  return (
    <button
      onClick={onToggle}
      className={cn(
        "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg border text-left transition-colors",
        checked
          ? "bg-primary/5 border-primary/20"
          : "bg-surface border-border hover:border-border/80",
        dimmed && !checked && "opacity-60",
      )}
    >
      <div
        className={cn(
          "w-4 h-4 rounded border-2 flex items-center justify-center shrink-0 transition-colors",
          checked ? "bg-primary border-primary" : "border-muted-foreground/40",
        )}
      >
        {checked && (
          <svg width="8" height="6" viewBox="0 0 8 6" fill="none">
            <path
              d="M1 3L3 5L7 1"
              stroke="white"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        )}
      </div>
      <BookOpen
        size={13}
        className={
          checked ? "text-primary shrink-0" : "text-muted-foreground shrink-0"
        }
      />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-medium text-foreground truncate">
          {subject.name}
        </p>
        <p className="text-[11px] text-muted-foreground">{subject.code}</p>
      </div>
    </button>
  );
}
