import { Plus, X } from "@phosphor-icons/react";

import { PropControl, type ControlProps } from "@/components/studio/components/inspector/PropControl";
import { NumberInput } from "@/components/studio/panel/fields";
import { Button } from "@/components/ui/button";
import type { PropSpec } from "@/lib/studio/components";
import { blankValue } from "@/lib/studio/props";

/** Numbers sit side by side, up to this many to a line (axis ranges, a
 *  point's coordinates), and wrap past it. */
const PER_LINE = 4;

/** The icon that adds an item to a list. */
export function AddItem({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <Button type="button" variant="ghost" size="icon-xs" aria-label={`Add to ${label}`} title="Add" onClick={onClick} className="text-muted-foreground">
      <Plus size={12} weight="bold" />
    </Button>
  );
}

/**
 * A list prop. Numbers are small fields in a grid, each with a remove badge
 * that shows on hover; anything else is a row per item with its remove at the
 * end, also on hover. Add is the last control.
 */
export function ListControl({
  spec,
  label,
  value,
  onChange,
  tokens,
}: ControlProps & { spec: Extract<PropSpec, { kind: "list" }> }) {
  const items = Array.isArray(value) ? value : [];
  const add = () => onChange([...items, blankValue(spec.of, tokens)]);
  const remove = (i: number) => onChange(items.filter((_, j) => j !== i));
  const edit = (i: number, v: unknown) => onChange(items.map((x, j) => (j === i ? v : x)));

  if (spec.of.kind === "num") {
    return (
      <div className="flex items-start gap-1">
        {items.length === 0 ? (
          <span className="flex min-h-7 flex-1 items-center text-[12px] text-muted-foreground">Empty</span>
        ) : (
          <div
            className="grid min-w-0 flex-1 gap-1"
            style={{ gridTemplateColumns: `repeat(${Math.min(PER_LINE, items.length)}, minmax(0, 1fr))` }}
          >
            {items.map((item, i) => (
              <div key={i} className="group/item relative">
                <NumberInput
                  label={`${label} ${i + 1}`}
                  value={typeof item === "number" ? item : undefined}
                  onChange={(n) => edit(i, n)}
                />
                <button
                  type="button"
                  aria-label={`Remove ${label} ${i + 1}`}
                  title="Remove"
                  onClick={() => remove(i)}
                  className="absolute -right-1 -top-1 z-10 flex size-3.5 cursor-pointer items-center justify-center rounded-full border border-border bg-popover text-muted-foreground opacity-0 outline-none hover:text-foreground focus-visible:opacity-100 group-hover/item:opacity-100"
                >
                  <X size={8} weight="bold" />
                </button>
              </div>
            ))}
          </div>
        )}
        <AddItem label={label} onClick={add} />
      </div>
    );
  }

  return (
    <div className="space-y-1">
      {items.map((item, i) => (
        <div key={i} className="group/item flex items-center gap-1">
          <div className="min-w-0 flex-1">
            <PropControl spec={spec.of} label={`${label} ${i + 1}`} value={item} tokens={tokens} onChange={(v) => edit(i, v)} />
          </div>
          <button
            type="button"
            aria-label={`Remove ${label} ${i + 1}`}
            title="Remove"
            onClick={() => remove(i)}
            className="flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted-foreground opacity-0 outline-none hover:bg-accent hover:text-foreground focus-visible:opacity-100 focus-visible:ring-[3px] focus-visible:ring-ring/25 group-hover/item:opacity-100"
          >
            <X size={11} />
          </button>
        </div>
      ))}
      <Button type="button" variant="ghost" size="xs" onClick={add} className="-ml-1 text-muted-foreground">
        <Plus size={11} weight="bold" />
        Add
      </Button>
    </div>
  );
}
