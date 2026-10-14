import { useEffect, useRef, type ReactNode } from "react";
import { ArrowDown, ArrowUp, CaretRight, Columns, Rows } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { GROUP_BYS, GROUP_LABELS, isGroupBy } from "../tasks/taskGroups";
import { SORT_KEYS, SORT_LABELS, isSortKey } from "../tasks/taskSort";
import { isDefaultDisplay, resetDisplay, type TaskLayout, type TasksDisplay } from "./settings";

const LAYOUTS: ReadonlyArray<{ value: TaskLayout; label: string; icon: typeof Rows }> = [
  { value: "table", label: "Table", icon: Rows },
  { value: "board", label: "Board", icon: Columns },
];

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-9 items-center justify-between gap-3 px-1">
      <span className="text-[12.5px] text-foreground">{label}</span>
      {children}
    </div>
  );
}

/**
 * The Display menu's first panel: layout, grouping, sort and completed tasks,
 * then a door to the property list. Grouping and properties draw the table
 * only, so the board hides both.
 */
export function DisplaySettings({
  display,
  onChange,
  active,
  onOpenProperties,
}: {
  display: TasksDisplay;
  onChange: (next: TasksDisplay) => void;
  /** Whether this panel is the one showing; coming back to it refocuses the
   *  Properties row, which the property panel was opened from. */
  active: boolean;
  onOpenProperties: () => void;
}) {
  const properties = useRef<HTMLButtonElement>(null);
  const wasActive = useRef(active);
  useEffect(() => {
    if (active && !wasActive.current) properties.current?.focus({ preventScroll: true });
    wasActive.current = active;
  }, [active]);
  const table = display.view === "table";
  const SortArrow = display.sort.dir === "asc" ? ArrowUp : ArrowDown;

  return (
    <div className="p-3">
      <Row label="View">
        <div role="radiogroup" aria-label="View" className="flex items-center gap-0.5 rounded-full bg-secondary p-0.5">
          {LAYOUTS.map(({ value, label, icon: LayoutIcon }) => {
            const selected = display.view === value;
            return (
              <button
                key={value}
                type="button"
                role="radio"
                aria-checked={selected}
                onClick={() => onChange({ ...display, view: value })}
                className={cn(
                  "flex h-6 cursor-pointer items-center gap-1.5 rounded-full px-2.5 text-xs font-medium outline-none transition-[color,background-color,box-shadow]",
                  "focus-visible:ring-[3px] focus-visible:ring-ring/50",
                  selected
                    ? "bg-card text-foreground shadow-xs"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                <LayoutIcon size={12} />
                {label}
              </button>
            );
          })}
        </div>
      </Row>

      {table && (
        <Row label="Grouping">
          <Select
            value={display.group}
            onValueChange={(group) => isGroupBy(group) && onChange({ ...display, group })}
          >
            <SelectTrigger aria-label="Group by" size="sm" className="h-7 w-36 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {GROUP_BYS.map((group) => (
                <SelectItem key={group} value={group} className="text-xs">
                  {GROUP_LABELS[group]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Row>
      )}

      {table && (
        <Row label="Group subtasks">
          <Switch aria-label="Group subtasks under parents" checked={display.groupSubtasks} onCheckedChange={(groupSubtasks) => onChange({ ...display, groupSubtasks })} />
        </Row>
      )}

      <Row label="Sort">
        <div className="flex items-center gap-1.5">
          <Select
            value={display.sort.key}
            onValueChange={(key) => isSortKey(key) && onChange({ ...display, sort: { ...display.sort, key } })}
          >
            <SelectTrigger aria-label="Sort by" size="sm" className="h-7 w-28 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {SORT_KEYS.map((key) => (
                <SelectItem key={key} value={key} className="text-xs">
                  {SORT_LABELS[key]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            type="button"
            variant="outline"
            size="icon-xs"
            aria-label={display.sort.dir === "asc" ? "Ascending: switch to descending" : "Descending: switch to ascending"}
            onClick={() =>
              onChange({ ...display, sort: { ...display.sort, dir: display.sort.dir === "asc" ? "desc" : "asc" } })
            }
            className="size-7"
          >
            <SortArrow size={12} weight="bold" />
          </Button>
        </div>
      </Row>

      <Row label="Show completed">
        <Switch
          aria-label="Show completed tasks"
          checked={display.showCompleted}
          onCheckedChange={(showCompleted) => onChange({ ...display, showCompleted })}
        />
      </Row>

      {table && (
        <button
          ref={properties}
          type="button"
          onClick={onOpenProperties}
          className="-mx-0.5 flex min-h-9 w-[calc(100%+0.25rem)] cursor-pointer items-center justify-between gap-3 rounded-md px-1.5 text-left outline-none hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          <span className="text-[12.5px] text-foreground">Properties</span>
          <span className="flex items-center gap-1 text-xs text-muted-foreground">
            <span className="tabular-nums">{display.properties.length + 1}</span>
            <CaretRight size={11} />
          </span>
        </button>
      )}

      <div className="mt-1.5 border-t border-border-subtle pt-2 pl-1">
        <button
          type="button"
          disabled={isDefaultDisplay(display)}
          onClick={() => onChange(resetDisplay())}
          className="cursor-pointer rounded disabled:cursor-default disabled:opacity-50 text-[11px] text-muted-foreground outline-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          Reset defaults
        </button>
      </div>
    </div>
  );
}
