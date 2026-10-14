import { useState } from "react";
import { CaretDown, Check } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { boardOf, type DbProject, type ProjectColumn } from "@/lib/planning/projects";
import { columnVariant, type ColumnVariant } from "../tasks/columnVariant";
import { TaskGlyph } from "../tasks/TaskMarks";

const PILL_CLASS: Record<ColumnVariant, string> = {
  backlog: "border-border bg-transparent text-muted-foreground",
  todo: "border-border bg-secondary text-foreground",
  doing: "border-brand/25 bg-brand-muted text-brand",
  done: "border-success/20 bg-success/15 text-success",
};

/** A column pill's tint, by {@link columnVariant}, so it agrees with the glyph.
 *  An unknown column is painted destructive. */
function columnPillClass(columns: ProjectColumn[], columnId: string): string {
  const variant = columnVariant(columns, columnId);
  return variant ? PILL_CLASS[variant] : "border-destructive/20 bg-destructive/10 text-destructive";
}

/**
 * A task's column as an editable pill. Callers write through `moveTask`, never
 * `updateTask`: only `moveTask` keeps `done_at` in step with the column.
 */
export function StatusPill({
  project,
  columnId,
  onPick,
  dot = false,
  chevron = false,
  className,
}: {
  /** `null` for an unfiled task (default board). */
  project: DbProject | null;
  columnId: string;
  onPick: (columnId: string) => void;
  /** A leading dot in the pill's own colour, for the task page's larger pill. */
  dot?: boolean;
  /** A quiet caret after the name, marking the pill as a dropdown. */
  chevron?: boolean;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const columns = boardOf(project);
  const column = columns.find((c) => c.id === columnId) ?? null;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Change status"
          className={cn(
            "inline-flex max-w-full cursor-pointer items-center rounded-full border px-2 py-0.5 text-[11px] font-medium transition-opacity hover:opacity-80",
            columnPillClass(columns, columnId),
            className,
          )}
        >
          {dot && <span aria-hidden className="mr-1.5 size-2 shrink-0 rounded-full bg-current" />}
          <span className="truncate">{column?.name ?? columnId}</span>
          {chevron && <CaretDown size={9} weight="bold" className="ml-1 shrink-0 opacity-60" />}
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-44 p-1">
        {columns.map((c) => (
          <button
            key={c.id}
            type="button"
            onClick={() => {
              setOpen(false);
              if (c.id !== columnId) onPick(c.id);
            }}
            className="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-foreground transition-colors hover:bg-accent"
          >
            <TaskGlyph variant={columnVariant(columns, c.id)} />
            <span className="min-w-0 flex-1 truncate">{c.name}</span>
            {c.id === columnId && <Check size={12} className="shrink-0 text-brand" />}
          </button>
        ))}
      </PopoverContent>
    </Popover>
  );
}
