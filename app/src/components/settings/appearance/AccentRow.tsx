import { Button } from "@/components/ui/button";
import { ColorField } from "@/components/ui/color/ColorField";
import { contrast, getTheme, setAppearance, useAppearance, type ThemeMode } from "@/lib/ui/themes";
import { AppearanceRow } from "./AppearanceRow";

/** Below this against a card, links and the ring blur into the page. */
const READABLE = 3;

/** The accent override. Shows the theme's own accent (of the mode in use)
 *  until one is set. */
export function AccentRow({ mode }: { mode: ThemeMode }) {
  const prefs = useAppearance();
  const value = prefs.accent ?? getTheme(prefs[mode], mode).palette.accent;
  const faint = prefs.accent
    ? (["light", "dark"] as const)
        .map((m) => getTheme(prefs[m], m))
        .filter((t) => contrast(prefs.accent!, t.palette.card) < READABLE)
        .map((t) => t.name)
    : [];

  return (
    <AppearanceRow
      label="Accent colour"
      description="Buttons, links and selection."
      note={faint.length ? `May be hard to read on ${faint.join(" and ")}.` : null}
    >
      {prefs.accent && (
        <Button variant="outline" size="xs" onClick={() => setAppearance({ accent: null })}>
          Use theme colour
        </Button>
      )}
      <ColorField label="Accent colour" value={value} opaque onChange={(hex) => setAppearance({ accent: hex })} />
    </AppearanceRow>
  );
}
