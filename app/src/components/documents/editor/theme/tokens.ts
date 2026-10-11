import type { EditorView } from "@codemirror/view";

/** One section of the note theme: selector to style, as `EditorView.theme` takes. */
export type ThemeSpec = Parameters<typeof EditorView.theme>[0];

export const brand = "var(--color-brand)";
export const muted = "var(--color-muted-foreground)";
export const mono = "var(--font-mono)";
