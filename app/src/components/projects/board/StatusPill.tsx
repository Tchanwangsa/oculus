import { useRef, useState } from "react";
import { CaretDown, Check } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
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
  glyphOnly = false,
  dot = false,
  chevron = false,
  className,
}: {
  /** `null` for an unfiled task (default board). */
  project: DbProject | null;
  columnId: string;
  onPick: (columnId: string) => void;
  /** Only the clickable status glyph, for the table's Name cell. */
  glyphOnly?: boolean;
  /** A leading dot in the pill's own colour, for the task page's larger pill. */
  dot?: boolean;
  /** A quiet caret after the name, marking the pill as a dropdown. */
  chevron?: boolean;
  className?: string;
}) {
  const listRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const columns = boardOf(project);
  const column = columns.find((c) => c.id === columnId) ?? null;

  useScrollFade(listRef, "y", open);
  const pick = (id: string) => {
    setOpen(false);
    if (id !== columnId) onPick(id);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Change status"
          title={column?.name ?? columnId}
          className={cn(
            "inline-flex max-w-full shrink-0 cursor-pointer items-center outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
            glyphOnly
              ? "size-5 justify-center rounded-full transition-colors hover:bg-accent"
              : "rounded-full border px-2 py-0.5 text-[11px] font-medium transition-opacity will-change-[opacity] hover:opacity-80",
            !glyphOnly && columnPillClass(columns, columnId),
            className,
          )}
        >
          {glyphOnly ? (
            <TaskGlyph variant={columnVariant(columns, columnId)} size={15} />
          ) : (
            <>
              {dot && <span aria-hidden className="mr-1.5 size-2 shrink-0 rounded-full bg-current" />}
              <span className="truncate">{column?.name ?? columnId}</span>
              {chevron && <CaretDown size={9} weight="bold" className="ml-1 shrink-0 opacity-60" />}
            </>
          )}
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-52 p-1"
        onOpenAutoFocus={(e) => {
          e.preventDefault();
          const list = listRef.current;
          const selected = list?.querySelector<HTMLButtonElement>('[aria-selected="true"]');
          (selected ?? list?.querySelector<HTMLButtonElement>('button'))?.focus();
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            const buttons = [...(listRef.current?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
            const index = buttons.findIndex((button) => button === document.activeElement);
            buttons[(index + (e.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
          } else if (/^[1-9]$/.test(e.key) && columns[Number(e.key) - 1]) {
            e.preventDefault();
            pick(columns[Number(e.key) - 1].id);
          }
        }}
      >
        <div ref={listRef} role="listbox" aria-label="Statuses" className="max-h-64 overflow-y-auto">
          {columns.map((c, index) => (
            <button
              key={c.id}
              type="button"
              role="option"
              aria-selected={c.id === columnId}
              onClick={() => pick(c.id)}
              className={cn("flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-2 text-left text-xs text-foreground outline-none transition-colors hover:bg-accent focus:bg-accent", c.id === columnId && "bg-accent")}
            >
              <TaskGlyph variant={columnVariant(columns, c.id)} />
              <span className="min-w-0 flex-1 truncate">{c.name}</span>
              {c.id === columnId && <Check size={12} className="shrink-0 text-brand" />}
              {index < 9 && <span className="w-3 shrink-0 text-right text-[11px] tabular-nums text-muted-foreground">{index + 1}</span>}
            </button>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}
