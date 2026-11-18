/** The compact settings menu's pages: the list of settings, and one page
 *  each that the list's rows open in its place. */
export type SettingsPage = "root" | "model" | "permission" | "usage";

/**
 * What a key does on a page: `back` to the list, or nothing (the key is the
 * page's controls'). Escape and Left go back from a page, Left only when no
 * text field or control took it (`editing`, `handled`); on the list Escape
 * is left to the popover, which closes.
 */
export function pageKey(page: SettingsPage, key: string, { editing = false, handled = false } = {}): "back" | null {
  if (page === "root" || handled) return null;
  if (key === "Escape") return "back";
  if (key === "ArrowLeft" && !editing) return "back";
  return null;
}

/** Which way a page slides in: a row's page from the right, the list back
 *  from the left. */
export function slideFrom(to: SettingsPage): "right" | "left" {
  return to === "root" ? "left" : "right";
}

/** The row to move to on the list for Up, Down, Home or End, wrapping; null
 *  for any other key. */
export function rowStep(key: string, at: number, count: number): number | null {
  if (count === 0) return null;
  if (key === "ArrowDown") return (at + 1) % count;
  if (key === "ArrowUp") return (at - 1 + count) % count;
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  return null;
}
