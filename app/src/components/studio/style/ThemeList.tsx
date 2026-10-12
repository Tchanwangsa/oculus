import { Check } from "@phosphor-icons/react";

import { LevelMark } from "@/components/studio/LevelMark";
import { Swatch } from "@/components/studio/style/Swatch";
import type { ThemesState } from "@/hooks/studio/useThemes";
import { cn } from "@/lib/utils";

/**
 * The themes to pick from, each with the library it comes from. The subject's
 * default (else the global one) is marked and chosen while nothing is picked
 * by hand; choosing it again goes back to following the default, so a later
 * subject change brings that subject's default.
 */
export function ThemeList({
  state,
  value,
  defaultTheme,
  onChange,
}: {
  state: ThemesState;
  /** The theme picked by hand; null follows the default. */
  value: string | null;
  /** The theme a new video in this subject takes, once known. */
  defaultTheme: string | null;
  onChange: (theme: string | null) => void;
}) {
  const shown = value ?? defaultTheme;
  return (
    <div role="listbox" aria-label="Theme" className="flex max-h-48 flex-col gap-0.5 overflow-y-auto">
      {defaultTheme == null && (
        <ThemeRow label="Default" note={<Note text="The subject's own" />} selected={value == null} onPick={() => onChange(null)} />
      )}
      {state.themes.map((t) => (
        <ThemeRow
          key={t.name}
          label={t.name}
          note={
            <>
              {t.name === defaultTheme && <Note text="Default" />}
              <LevelMark level={t.level} shadows={t.shadows != null} inSubject />
            </>
          }
          colors={t.swatch}
          selected={shown === t.name}
          disabled={t.problem != null}
          title={t.problem ?? undefined}
          onPick={() => onChange(t.name === defaultTheme ? null : t.name)}
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

function Note({ text }: { text: string }) {
  return <span className="shrink-0 text-[10.5px] font-medium text-brand">{text}</span>;
}

function ThemeRow({
  label,
  note,
  colors,
  selected,
  disabled = false,
  title,
  onPick,
}: {
  label: string;
  note: React.ReactNode;
  colors?: string[];
  selected: boolean;
  disabled?: boolean;
  title?: string;
  onPick: () => void;
}) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      disabled={disabled}
      title={title}
      onClick={onPick}
      className={cn(
        "flex w-full cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12.5px] transition-colors disabled:cursor-default disabled:opacity-50",
        selected ? "bg-accent text-foreground" : "text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      {colors ? <Swatch colors={colors} /> : <span className="w-6 shrink-0" />}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {note}
      <Check size={12} weight="bold" className={cn("shrink-0", !selected && "invisible")} />
    </button>
  );
}
