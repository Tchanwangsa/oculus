import type { EditorView } from "@codemirror/view";

/** One section of the note theme: selector to style, as `EditorView.theme` takes. */
export type ThemeSpec = Parameters<typeof EditorView.theme>[0];

/** The element a tooltip's rules hang from: the editor, or the container
 *  CodeMirror makes for `tooltips({ parent })`, which has the theme class but
 *  not `cm-editor`. `:not()` weighs one class, as `.cm-editor` did, so the
 *  rules still beat the base theme's. */
export const tooltipRoot = "&:not(.cm-tooltip-free)";

export const brand = "var(--color-brand)";
export const muted = "var(--color-muted-foreground)";
export const mono = "var(--font-mono)";
