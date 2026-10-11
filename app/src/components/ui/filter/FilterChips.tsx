import { useRef } from "react";
import { X } from "@phosphor-icons/react";
import { useScrollFade } from "@/hooks/ui/useScrollFade";

export interface FilterChip {
  key: string;
  label: string;
  onRemove: () => void;
}

/**
 * The applied filters under a toolbar, one removable pill each, on a single
 * line that scrolls sideways with faded edges rather than wrapping. Renders
 * nothing while no filter is applied.
 */
export function FilterChips({ chips, onClear }: { chips: readonly FilterChip[]; onClear: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "x", chips.length > 0);
  if (chips.length === 0) return null;

  return (
    <div className="flex shrink-0 items-center gap-2 px-5 pb-2.5">
      <div
        ref={ref}
        className="min-w-0 flex-1 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        <div className="flex w-max items-center gap-1.5">
          {chips.map((c) => (
            <span
              key={c.key}
              className="flex h-6 shrink-0 items-center gap-1 rounded-full bg-brand/10 pr-1 pl-2.5 text-[11px] text-brand"
            >
              {c.label}
              <button
                type="button"
                aria-label={`Remove ${c.label}`}
                onClick={c.onRemove}
                className="flex size-4 cursor-pointer items-center justify-center rounded-full transition-colors hover:bg-brand/15"
              >
                <X size={9} weight="bold" />
              </button>
            </span>
          ))}
        </div>
      </div>
      <button
        type="button"
        onClick={onClear}
        className="shrink-0 cursor-pointer text-[11px] text-muted-foreground transition-colors hover:text-foreground"
      >
        Clear all
      </button>
    </div>
  );
}
