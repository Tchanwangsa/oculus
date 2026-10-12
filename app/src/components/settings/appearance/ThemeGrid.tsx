import { useRef, type KeyboardEvent } from "react";

import { setAppearance, themesFor, useAppearance, type ThemeMode } from "@/lib/ui/themes";
import { ThemeCard } from "./ThemeCard";

const LABEL: Record<ThemeMode, string> = { light: "Light theme", dark: "Dark theme" };
const STEP: Record<string, number> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };

/** The themes of one mode as a radio group of cards. `inUse` marks the
 *  mode the window is showing now. */
export function ThemeGrid({ mode, inUse }: { mode: ThemeMode; inUse: boolean }) {
  const prefs = useAppearance();
  const themes = themesFor(mode);
  const cards = useRef<(HTMLButtonElement | null)[]>([]);
  const chosen = Math.max(0, themes.findIndex((t) => t.id === prefs[mode]));

  const choose = (i: number) => setAppearance(mode === "light" ? { light: themes[i].id } : { dark: themes[i].id });

  // Radio-group keys: arrows move the choice and the focus together.
  const onKeyDown = (e: KeyboardEvent) => {
    const step = STEP[e.key];
    if (!step) return;
    e.preventDefault();
    const next = (chosen + step + themes.length) % themes.length;
    choose(next);
    cards.current[next]?.focus();
  };

  return (
    <div className="py-2">
      <p className="flex items-center gap-1.5 text-xs text-foreground">
        {LABEL[mode]}
        {inUse && <span className="text-[11px] text-muted-foreground">In use</span>}
      </p>
      <div
        role="radiogroup"
        aria-label={LABEL[mode]}
        onKeyDown={onKeyDown}
        className="mt-2 grid gap-3 [grid-template-columns:repeat(auto-fill,minmax(168px,1fr))]"
      >
        {themes.map((theme, i) => (
          <ThemeCard
            key={theme.id}
            ref={(el) => {
              cards.current[i] = el;
            }}
            theme={theme}
            accent={prefs.accent}
            selected={i === chosen}
            onSelect={() => choose(i)}
          />
        ))}
      </div>
    </div>
  );
}
