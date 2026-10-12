import { accentTokens } from "./accent";
import { mix } from "./oklab";
import { SYNTAX_SLOTS, type SyntaxColors, type ThemeMode, type ThemeSpec, type TokenMap } from "./types";

/** Every step below is a fraction of the way from one palette colour to
 *  another in OKLab, fitted to `colors.css` (light) and `dark.css` (dark) so
 *  a derived theme keeps the stylesheet's spacing. */
interface Steps {
  /** ink-50 to ink-900, from the far end of the ramp to the foreground. */
  ink: number[];
  /** From the card towards the foreground. */
  surface: number;
  raised: number;
  overlay: number;
  input: number;
  /** From the ground towards the foreground: the rail's and tabs' pills. */
  itemHover: number;
  itemActive: number;
  /** From the card towards the border. */
  borderSubtle: number;
}

const STEPS: Record<ThemeMode, Steps> = {
  light: {
    ink: [0, 0.021, 0.039, 0.068, 0.111, 0.274, 0.52, 0.703, 0.845, 1],
    surface: 0.039,
    raised: 0.068,
    overlay: 0.111,
    input: 0.118,
    itemHover: 0.047,
    itemActive: 0.081,
    borderSubtle: 0.6,
  },
  dark: {
    ink: [0, 0.041, 0.058, 0.109, 0.21, 0.452, 0.683, 0.855, 0.951, 1],
    surface: 0.03,
    raised: 0.083,
    overlay: 0.145,
    input: 0.161,
    itemHover: 0.087,
    itemActive: 0.136,
    borderSubtle: 0.35,
  },
};

const STATUS: Record<ThemeMode, { destructive: string; success: string; warning: string }> = {
  light: { destructive: "#dc2626", success: "#16a34a", warning: "#b45309" },
  dark: { destructive: "#ef4444", success: "#22c55e", warning: "#f59e0b" },
};

const INK = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900];

export function syntaxTokens(syntax: SyntaxColors): TokenMap {
  return Object.fromEntries(SYNTAX_SLOTS.map((slot) => [`--color-syntax-${slot}`, syntax[slot]]));
}

/** The full `--color-*` record for a theme, chart slots and shadows aside
 *  (the stylesheet's per-mode values serve every theme). */
export function derive(spec: ThemeSpec): TokenMap {
  const { palette: p, mode } = spec;
  const s = STEPS[mode];
  const fg = p.foreground;
  const toFg = (from: string, t: number) => mix(from, fg, t);
  // The ramp's far end is the lightest surface in light mode, the darkest in dark.
  const base = mode === "light" ? p.card : p.ground;
  const status = STATUS[mode];
  const raised = toFg(p.card, s.raised);
  const surface = toFg(p.card, s.surface);

  const ink = Object.fromEntries(
    INK.map((step, i) => [
      `--color-ink-${step}`,
      step === 600 ? p.mutedForeground : toFg(base, s.ink[i]),
    ]),
  );

  return {
    ...ink,
    "--color-background": p.ground,
    "--color-foreground": fg,
    "--color-card": p.card,
    "--color-card-foreground": fg,
    "--color-popover": p.popover,
    "--color-popover-foreground": fg,
    "--color-surface": surface,
    "--color-surface-raised": raised,
    "--color-surface-overlay": toFg(p.card, s.overlay),
    "--color-border": p.border,
    "--color-border-subtle": mix(p.card, p.border, s.borderSubtle),
    "--color-input": toFg(p.card, s.input),
    "--color-muted": surface,
    "--color-muted-foreground": p.mutedForeground,
    ...accentTokens(p.accent, p, mode),
    "--color-secondary": raised,
    "--color-secondary-foreground": fg,
    "--color-accent": raised,
    "--color-accent-foreground": fg,
    "--color-destructive": p.destructive ?? status.destructive,
    "--color-destructive-foreground": "#ffffff",
    "--color-success": p.success ?? status.success,
    "--color-warning": p.warning ?? status.warning,
    "--color-sidebar": p.ground,
    "--color-sidebar-border": p.border,
    "--color-sidebar-item-hover": toFg(p.ground, s.itemHover),
    "--color-sidebar-item-active": toFg(p.ground, s.itemActive),
    ...syntaxTokens(spec.syntax),
  };
}

/** What applying the theme writes inline: nothing for a built-in, whose
 *  values are the stylesheet's. */
export function themeTokens(spec: ThemeSpec): TokenMap {
  return spec.builtin ? {} : derive(spec);
}
