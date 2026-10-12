/** Desktop platform and the labels shown for shared keyboard shortcuts. */
export const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
export const isWindows = typeof navigator !== "undefined" && /Win/.test(navigator.platform);

/** Uses the same modifier names as editor keymaps and native menu shortcuts. */
export function shortcut(keys: string, mac = isMac): string {
  const parts = keys.split("+").map((part) => {
    switch (part.toLowerCase()) {
      case "mod": return mac ? "⌘" : "Ctrl";
      case "ctrl": return mac ? "⌃" : "Ctrl";
      case "shift": return mac ? "⇧" : "Shift";
      case "alt": return mac ? "⌥" : "Alt";
      case "enter": return mac ? "↵" : "Enter";
      case "space": return "Space";
      default: return part.length === 1 ? part.toUpperCase() : part;
    }
  });
  return parts.join(mac ? "" : "+");
}

export const sidePanelShortcut = isWindows ? "mod+shift+Y" : "alt+mod+T";
