/** Native chrome and shortcut labels share one platform decision. The webview
 * exposes its host OS here in both development and packaged builds. */
export const isMac =
  typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

export const isWindows =
  typeof navigator !== "undefined" && /Win/.test(navigator.platform);

export const primaryModifier = isMac ? "⌘" : "Ctrl";

export function shortcut(key: string, alt = false): string {
  if (isMac) return `${alt ? "⌥" : ""}⌘${key === "Enter" ? "↵" : key}`;
  return `Ctrl+${alt ? "Alt+" : ""}${key}`;
}
