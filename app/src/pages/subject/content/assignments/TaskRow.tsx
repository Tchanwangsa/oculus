import { PencilLine, Rocket } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { fmtDue, type TaskDoc } from "./grouping";

export function TaskRow({ task: t, muted }: { task: TaskDoc; muted: boolean }) {
  const Icon = t.kind === "quiz" ? Rocket : PencilLine;
  return (
    <button
      data-tab-href={filePageHref(t.file) ?? undefined}
      onClick={() => openFileSmart(t.file)}
      className={cn(
        "w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-surface transition-colors",
        muted && "opacity-70",
      )}
    >
      <Icon size={13} className="shrink-0 opacity-60" />
      <span className="min-w-0 flex-1">
        <span className="block text-[12px] text-foreground truncate">
          {t.title}
        </span>
        <span className="block text-[11px] text-muted-foreground truncate">
          {t.due ? `Due ${fmtDue(t.due)}` : "No due date"}
          {t.points != null && ` · ${t.points} pts`}
        </span>
      </span>
      <FileRecency file={t.file} />
    </button>
  );
}
