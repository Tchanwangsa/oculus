import { Checkbox } from "@/components/ui/checkbox";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { cn } from "@/lib/utils";
import { displayCode, displayName, fmtSynced } from "@/lib/format";
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
        "w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg border text-left transition-colors cursor-pointer",
        checked
          ? "bg-primary/5 border-primary/20"
          : "bg-surface border-border hover:border-border/80",
        dimmed && !checked && "opacity-60",
      )}
    >
      {/* The row itself is the control, so the checkbox is presentational —
          pointer-events-none keeps it from swallowing the row's click. */}
      <Checkbox
        checked={checked}
        tabIndex={-1}
        aria-hidden
        className="pointer-events-none shrink-0 size-3.5 [&_svg]:size-2.5"
      />
      <SubjectIcon code={subject.code} size={14} />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-medium text-foreground truncate">
          {displayName(subject.name, subject.code)}
        </p>
        <p className="text-[10px] text-muted-foreground truncate">
          {displayCode(subject.code)} · {fmtSynced(subject.last_synced_at)}
        </p>
      </div>
    </button>
  );
}
