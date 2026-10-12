// Themes: a registry of palettes, `derive` to turn one into every
// `--color-*` token, and the appearance prefs that pick and apply them
// (docs/ui.md).

export type { SyntaxColors, SyntaxSlot, ThemeMode, ThemePalette, ThemeSpec, TokenMap } from "./types";
export { SYNTAX_SLOTS } from "./types";
export { derive, themeTokens } from "./derive";
export { accentTokens, textOn } from "./accent";
export { contrast, mix } from "./oklab";
export { BUILTIN, THEMES, getTheme, hasTheme, themesFor } from "./registry";
export type { AppearanceMode, AppearancePrefs, Frame } from "./prefs";
export { defaultAppearance, normalizeAccent, readAppearance, validateAppearance } from "./prefs";
export { FRAME_ALPHA, MONO_STACK, appearanceTokens, applyAppearance, prefersDark, resolveMode } from "./apply";
export { getAppearance, setAppearance, subscribeAppearance, useAppearance, watchSystemTheme } from "./store";
export { isDark, subscribeDark, subscribeTheme, themeKey } from "./observe";
