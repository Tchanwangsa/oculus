import type { Ref } from "react";
import { CheckCircle } from "@phosphor-icons/react";

import type { ThemeSpec } from "@/lib/ui/themes";
import { cn } from "@/lib/utils";
import { ThemeSketch } from "./ThemeSketch";

/** One theme as a radio: its sketch, then its name. */
export function ThemeCard({
  theme,
  accent,
  selected,
  onSelect,
  ref,
}: {
  theme: ThemeSpec;
  accent: string | null;
  selected: boolean;
  onSelect: () => void;
  ref?: Ref<HTMLButtonElement>;
}) {
  return (
    <button
      ref={ref}
      type="button"
      role="radio"
      aria-checked={selected}
      aria-label={theme.name}
      // Roving focus: Tab lands on the chosen card, arrows move (ThemeGrid).
      tabIndex={selected ? 0 : -1}
      onClick={onSelect}
      className={cn(
        "flex min-w-0 cursor-pointer flex-col overflow-hidden rounded-lg border bg-card text-left outline-none",
        "transition-[border-color,box-shadow] focus-visible:ring-[3px] focus-visible:ring-ring/50",
        selected ? "border-brand ring-1 ring-brand" : "border-border hover:border-ink-500",
      )}
    >
      <ThemeSketch theme={theme} accent={accent} />
      <span className="flex items-center gap-1.5 border-t border-border px-2 py-1.5">
        <span className="truncate text-xs text-foreground">{theme.name}</span>
        {theme.builtin && <span className="shrink-0 text-[11px] text-muted-foreground">Default</span>}
        {selected && <CheckCircle size={14} weight="fill" className="ml-auto shrink-0 text-brand" />}
      </span>
    </button>
  );
}
