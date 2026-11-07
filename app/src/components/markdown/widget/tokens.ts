/** The tokens the kit's stylesheet reads (`widget-kit/kit.css`). */
const NAMES = [
  "background", "foreground", "card", "card-foreground", "popover", "popover-foreground",
  "surface", "surface-raised", "surface-overlay", "border", "border-subtle", "input", "ring",
  "muted", "muted-foreground", "primary", "primary-hover", "primary-active", "primary-foreground",
  "brand", "brand-hover", "brand-muted", "brand-foreground", "secondary", "secondary-foreground",
  "accent", "accent-foreground", "destructive", "destructive-foreground", "success", "warning",
  "chart-1", "chart-2", "chart-3", "chart-4", "chart-5", "chart-other",
].map((n) => `--color-${n}`);

/** The applied theme's colours as the page resolves them: the stylesheet's,
 *  or the ones `applyAppearance` writes inline on `<html>`. */
export function readTokens(): Record<string, string> {
  const style = getComputedStyle(document.documentElement);
  const tokens: Record<string, string> = {};
  for (const name of NAMES) {
    const value = style.getPropertyValue(name).trim();
    if (value) tokens[name] = value;
  }
  return tokens;
}
