import { Swatch } from "@/components/studio/style/Swatch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { ThemesState } from "@/hooks/studio/useThemes";

/** The theme the stage draws in, each with its colours; one that does not
 *  parse is listed but cannot be picked. */
export function ThemeSelect({
  state,
  value,
  onChange,
}: {
  state: ThemesState;
  value: string;
  onChange: (name: string) => void;
}) {
  const active = state.themes.find((t) => t.name === value) ?? null;
  return (
    <Select value={active ? value : ""} onValueChange={onChange} disabled={state.themes.length === 0}>
      <SelectTrigger size="sm" aria-label="Theme" className="h-7 gap-2 text-xs">
        <SelectValue placeholder={state.loading ? "Themes…" : "No theme"}>
          {active && (
            <span className="flex items-center gap-2">
              <Swatch colors={active.swatch} />
              {active.name}
            </span>
          )}
        </SelectValue>
      </SelectTrigger>
      <SelectContent align="end" className="max-h-64 min-w-[10rem]">
        {state.themes.map((t) => (
          <SelectItem key={t.name} value={t.name} disabled={t.problem != null} title={t.problem ?? undefined} className="text-xs">
            <span className="flex items-center gap-2">
              <Swatch colors={t.swatch} />
              {t.name}
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
