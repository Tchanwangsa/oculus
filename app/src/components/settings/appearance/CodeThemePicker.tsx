import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { getTheme, setAppearance, themesFor, useAppearance, type ThemeMode } from "@/lib/ui/themes";
import { CodePreview } from "./CodePreview";

const LABEL: Record<ThemeMode, string> = { light: "Light", dark: "Dark" };

/** One mode's code theme: "Match theme" or any theme of that mode, with the
 *  preview under it. */
export function CodeThemePicker({ mode }: { mode: ThemeMode }) {
  const prefs = useAppearance();
  const choice = mode === "light" ? prefs.codeLight : prefs.codeDark;
  const app = getTheme(prefs[mode], mode);
  const code = choice === "match" ? app : getTheme(choice, mode);
  const choose = (id: string) => setAppearance(mode === "light" ? { codeLight: id } : { codeDark: id });

  return (
    <div className="min-w-0 space-y-2">
      <p className="text-xs text-foreground">{LABEL[mode]}</p>
      <Select value={choice} onValueChange={choose}>
        <SelectTrigger aria-label={`${LABEL[mode]} code theme`} size="sm" className="h-7 w-full text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="match" className="text-xs">
            Match theme
          </SelectItem>
          {themesFor(mode).map((theme) => (
            <SelectItem key={theme.id} value={theme.id} className="text-xs">
              {theme.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <CodePreview app={app} code={code} />
    </div>
  );
}
