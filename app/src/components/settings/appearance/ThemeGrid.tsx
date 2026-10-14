import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";

import { useStoredState } from "@/hooks/ui/useStoredState";
import { setAppearance, themesFor, useAppearance, type ThemeMode } from "@/lib/ui/themes";
import { ThemeCard } from "./ThemeCard";

const LABEL: Record<ThemeMode, string> = { light: "Light theme", dark: "Dark theme" };
const STEP: Record<string, number> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };
/** The grid's `minmax` floor and `gap-3`, for counting its columns. */
const CARD_MIN = 168;
const GAP = 12;

const readExpanded = (raw: string | null) => raw === "1";
const writeExpanded = (open: boolean) => (open ? "1" : "0");

/** Columns `repeat(auto-fill, minmax(CARD_MIN, 1fr))` makes in `width`. */
function columns(width: number): number {
  return Math.max(1, Math.floor((width + GAP) / (CARD_MIN + GAP)));
}

/**
 * The themes of one mode as a radio group of cards: one row until "See all"
 * opens the rest (remembered). Folded, the chosen theme takes the row's last
 * slot when it would fall past it, so the choice always shows. `inUse` marks
 * the mode the window is showing now.
 */
export function ThemeGrid({ mode, inUse }: { mode: ThemeMode; inUse: boolean }) {
  const prefs = useAppearance();
  const themes = themesFor(mode);
  const [expanded, setExpanded] = useStoredState(`oculus-appearance-grid-${mode}`, readExpanded, writeExpanded);
  const [cols, setCols] = useState(themes.length);
  const grid = useRef<HTMLDivElement>(null);
  const cards = useRef(new Map<string, HTMLButtonElement>());
  const focusNext = useRef<string | null>(null);
  const chosen = Math.max(0, themes.findIndex((t) => t.id === prefs[mode]));

  // Before paint, so the folded row never flashes the whole grid.
  useLayoutEffect(() => {
    const el = grid.current;
    if (!el) return;
    const measure = () => setCols(columns(el.clientWidth));
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const folds = themes.length > cols;
  const visible =
    expanded || !folds
      ? themes
      : chosen < cols
        ? themes.slice(0, cols)
        : [...themes.slice(0, cols - 1), themes[chosen]];

  // A key press can swap which card holds the last slot; focus follows the
  // choice to its new element once it renders.
  useEffect(() => {
    if (!focusNext.current) return;
    cards.current.get(focusNext.current)?.focus();
    focusNext.current = null;
  });

  const choose = (id: string) => setAppearance(mode === "light" ? { light: id } : { dark: id });

  // Radio-group keys cycle every theme; folded, the choice stays in view.
  const onKeyDown = (e: KeyboardEvent) => {
    const step = STEP[e.key];
    if (!step) return;
    e.preventDefault();
    const next = themes[(chosen + step + themes.length) % themes.length].id;
    focusNext.current = next;
    choose(next);
  };

  return (
    <div className="py-2">
      <div className="flex flex-wrap items-center gap-1.5">
        <p className="text-xs text-foreground">{LABEL[mode]}</p>
        {inUse && <span className="text-[11px] text-muted-foreground">In use</span>}
        {folds && (
          <button
            type="button"
            aria-expanded={expanded}
            onClick={() => setExpanded(!expanded)}
            className="ml-auto cursor-pointer rounded-full px-2 py-0.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
          >
            {expanded ? "Show less" : `See all ${themes.length}`}
          </button>
        )}
      </div>
      <div
        ref={grid}
        role="radiogroup"
        aria-label={LABEL[mode]}
        onKeyDown={onKeyDown}
        className="mt-2 grid gap-3 [grid-template-columns:repeat(auto-fill,minmax(min(168px,100%),1fr))]"
      >
        {visible.map((theme) => (
          <ThemeCard
            key={theme.id}
            ref={(el) => {
              if (el) cards.current.set(theme.id, el);
              else cards.current.delete(theme.id);
            }}
            theme={theme}
            accent={prefs.accent}
            selected={theme.id === themes[chosen].id}
            onSelect={() => choose(theme.id)}
          />
        ))}
      </div>
    </div>
  );
}
