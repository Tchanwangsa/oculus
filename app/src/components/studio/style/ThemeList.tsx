import { Check } from "@phosphor-icons/react";

import { Swatch } from "@/components/studio/style/Swatch";
import type { ThemesState } from "@/hooks/studio/useThemes";
import { cn } from "@/lib/utils";

const LEVEL_LABEL = { bundled: "", global: "Global", subject: "Subject" } as const;

/** The themes to pick from. The first row is "Default": no theme named, so
 *  the video takes its subject's default. */
export function ThemeList({
  state,
  value,
  onChange,
}: {
  state: ThemesState;
  value: string | null;
  onChange: (theme: string | null) => void;
}) {
  return (
    <div role="listbox" aria-label="Theme" className="flex max-h-48 flex-col gap-0.5 overflow-y-auto">
      <ThemeRow label="Default" note="The subject's own" selected={value == null} onPick={() => onChange(null)} />
      {state.themes.map((t) => (
        <ThemeRow
          key={t.name}
          label={t.name}
          note={LEVEL_LABEL[t.level]}
          colors={t.swatch}
          selected={value === t.name}
          onPick={() => onChange(t.name)}
        />
      ))}
      {state.error && (
        <p className="px-2 py-1 text-[11px] leading-snug text-muted-foreground">
          Themes could not be listed: {state.error}
        </p>
      )}
    </div>
  );
}

function ThemeRow({
  label,
  note,
  colors,
  selected,
  onPick,
}: {
  label: string;
  note: string;
  colors?: string[];
  selected: boolean;
  onPick: () => void;
}) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      onClick={onPick}
      className={cn(
        "flex w-full cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12.5px] transition-colors",
        selected ? "bg-accent text-foreground" : "text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      {colors ? <Swatch colors={colors} /> : <span className="w-6 shrink-0" />}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {note && <span className="shrink-0 text-[10.5px] text-muted-foreground">{note}</span>}
      <Check size={12} weight="bold" className={cn("shrink-0", !selected && "invisible")} />
    </button>
  );
}
