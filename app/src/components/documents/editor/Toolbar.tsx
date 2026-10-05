import { useState, type ComponentType, type ReactNode } from "react";
import type { StateCommand } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import {
  CodeBlock,
  CodeSimple,
  Image,
  LinkSimple,
  ListBullets,
  ListChecks,
  ListNumbers,
  Minus,
  Quotes,
  Sigma,
  Table,
  TextB,
  TextHOne,
  TextHThree,
  TextHTwo,
  TextItalic,
  TextStrikethrough,
  TextT,
  type IconProps,
} from "@phosphor-icons/react";

import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

import {
  insertDivider,
  insertMath,
  insertTable,
  setBlockType,
  toggleBold,
  toggleCode,
  toggleCodeBlock,
  toggleItalic,
  toggleLink,
  toggleList,
  toggleStrike,
  type ActiveFormats,
  type BlockType,
} from "./commands";

const BLOCKS: { value: BlockType; label: string; Icon: ComponentType<IconProps> }[] = [
  { value: "p", label: "Text", Icon: TextT },
  { value: "h1", label: "Heading 1", Icon: TextHOne },
  { value: "h2", label: "Heading 2", Icon: TextHTwo },
  { value: "h3", label: "Heading 3", Icon: TextHThree },
];

/** Largest table the grid offers. */
const GRID_ROWS = 8;
const GRID_COLS = 6;

/** Keeps the editor focused: a button that took focus would blur it, and the
 *  blur would drop the selection the command acts on from view. */
const keepFocus = (e: React.MouseEvent) => e.preventDefault();

/**
 * Formatting for the note, sticky under the title unless `className` places
 * it (merged with `cn`, so it overrides). Every button is a command
 * from `commands.ts` that rewrites markdown text, so it works in both modes.
 * Active states follow the syntax tree at the main selection (`active`).
 */
export function Toolbar({
  ref,
  view,
  active,
  onImage,
  className,
}: {
  ref?: React.Ref<HTMLDivElement>;
  view: EditorView | null;
  active: ActiveFormats;
  onImage: () => void;
  className?: string;
}) {
  const run = (command: StateCommand) => {
    if (!view) return;
    command(view);
    view.focus();
  };

  const heading = BLOCKS.find((b) => b.value === active.block);

  return (
    <div
      ref={ref}
      className={cn(
        "sticky top-0 z-10 -mx-2 mt-3 flex flex-wrap items-center gap-0.5 border-b border-border-subtle bg-card px-2 py-1.5",
        className,
      )}
    >
      <Select
        value={heading ? active.block : ""}
        onValueChange={(v) => run(setBlockType(v as BlockType))}
      >
        <SelectTrigger
          size="sm"
          aria-label="Block type"
          className="h-7 w-32 rounded-md border-0 px-2 text-[12px] shadow-none"
          onMouseDown={keepFocus}
        >
          <SelectValue placeholder={`Heading ${active.block.slice(1)}`} />
        </SelectTrigger>
        <SelectContent
          onCloseAutoFocus={(e) => {
            e.preventDefault();
            view?.focus();
          }}
        >
          {BLOCKS.map((b) => (
            <SelectItem key={b.value} value={b.value}>
              <b.Icon />
              {b.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Divider />
      <Tool label="Bold" keys="⌘B" on={active.bold} onRun={() => run(toggleBold)}>
        <TextB weight="bold" />
      </Tool>
      <Tool label="Italic" keys="⌘I" on={active.italic} onRun={() => run(toggleItalic)}>
        <TextItalic />
      </Tool>
      <Tool label="Strikethrough" keys="⌘⇧X" on={active.strike} onRun={() => run(toggleStrike)}>
        <TextStrikethrough />
      </Tool>
      <Tool label="Inline code" keys="⌘E" on={active.code} onRun={() => run(toggleCode)}>
        <CodeSimple />
      </Tool>
      <Tool label="Link" keys="⌘K" on={active.link} onRun={() => run(toggleLink)}>
        <LinkSimple />
      </Tool>

      <Divider />
      <Tool label="Bulleted list" on={active.bullet} onRun={() => run(toggleList("bullet"))}>
        <ListBullets />
      </Tool>
      <Tool label="Numbered list" on={active.ordered} onRun={() => run(toggleList("ordered"))}>
        <ListNumbers />
      </Tool>
      <Tool label="Checklist" on={active.task} onRun={() => run(toggleList("task"))}>
        <ListChecks />
      </Tool>
      <Tool label="Quote" on={active.quote} onRun={() => run(toggleList("quote"))}>
        <Quotes />
      </Tool>

      <Divider />
      <Tool label="Code block" on={active.codeBlock} onRun={() => run(toggleCodeBlock)}>
        <CodeBlock />
      </Tool>
      <Tool label="Equation" on={active.math} onRun={() => run(insertMath)}>
        <Sigma />
      </Tool>
      <TablePicker onPick={(rows, cols) => run(insertTable(rows, cols))} onClose={() => view?.focus()} />
      <Tool label="Image" onRun={onImage}>
        <Image />
      </Tool>
      <Tool label="Divider" onRun={() => run(insertDivider)}>
        <Minus />
      </Tool>
    </div>
  );
}

function Divider() {
  return <span aria-hidden className="mx-1 h-4 w-px bg-border" />;
}

/** The toolbar's button: a rectangle in a segmented bar, not a pill. */
const toolClass = (on?: boolean) =>
  cn(
    "inline-flex size-7 cursor-pointer items-center justify-center rounded-md transition-colors [&_svg]:size-[15px]",
    on
      ? "bg-accent text-foreground"
      : "text-muted-foreground hover:bg-accent hover:text-foreground",
  );

function Tool({
  label,
  keys,
  on,
  onRun,
  children,
}: {
  label: string;
  keys?: string;
  on?: boolean;
  onRun: () => void;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          aria-pressed={on}
          onMouseDown={keepFocus}
          onClick={onRun}
          className={toolClass(on)}
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        {label}
        {keys && <span className="ml-1.5 opacity-60">{keys}</span>}
      </TooltipContent>
    </Tooltip>
  );
}

/** A rows × columns grid; the hovered cell sets the size, header included. */
function TablePicker({
  onPick,
  onClose,
}: {
  onPick: (rows: number, cols: number) => void;
  onClose: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [size, setSize] = useState({ rows: 3, cols: 3 });

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label="Table"
              onMouseDown={keepFocus}
              className={toolClass(open)}
            >
              <Table />
            </button>
          </PopoverTrigger>
        </TooltipTrigger>
        <TooltipContent side="bottom">Table</TooltipContent>
      </Tooltip>
      <PopoverContent
        align="start"
        className="w-auto p-2"
        onCloseAutoFocus={(e) => {
          e.preventDefault();
          onClose();
        }}
      >
        <div
          className="grid gap-0.5"
          style={{ gridTemplateColumns: `repeat(${GRID_COLS}, 1rem)` }}
          onMouseLeave={() => setSize({ rows: 3, cols: 3 })}
        >
          {Array.from({ length: GRID_ROWS * GRID_COLS }, (_, i) => {
            const row = Math.floor(i / GRID_COLS) + 1;
            const col = (i % GRID_COLS) + 1;
            const lit = row <= size.rows && col <= size.cols;
            return (
              <button
                key={i}
                type="button"
                aria-label={`${row} by ${col} table`}
                onMouseEnter={() => setSize({ rows: row, cols: col })}
                onFocus={() => setSize({ rows: row, cols: col })}
                onClick={() => {
                  setOpen(false);
                  onPick(row, col);
                }}
                className={cn(
                  "size-4 cursor-pointer rounded-[3px] border transition-colors",
                  lit ? "border-brand bg-brand-muted" : "border-border bg-card",
                )}
              />
            );
          })}
        </div>
        <p className="mt-1.5 text-center text-[11px] text-muted-foreground tabular-nums">
          {size.rows} × {size.cols}
        </p>
      </PopoverContent>
    </Popover>
  );
}
