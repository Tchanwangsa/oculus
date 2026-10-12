import type { ThemeSpec } from "./types";

/** `colors.css` as a spec. Applying it writes nothing; keep these values in
 *  step with the stylesheet so a preview draws what the app shows. */
export const OCULUS_LIGHT: ThemeSpec = {
  id: "oculus-light",
  name: "Oculus Light",
  mode: "light",
  builtin: true,
  palette: {
    ground: "#f9f9f9",
    card: "#ffffff",
    popover: "#ffffff",
    foreground: "#0d0d0d",
    mutedForeground: "#757575",
    border: "#e5e5e5",
    accent: "#5e6ad2",
    destructive: "#dc2626",
    success: "#16a34a",
    warning: "#b45309",
  },
  syntax: {
    keyword: "#b4405e",
    string: "#2a7350",
    comment: "#737373",
    number: "#9a540b",
    function: "#6e4fb8",
    type: "#1b7183",
    property: "#35609b",
    operator: "#5c5c5c",
    meta: "#7e6129",
  },
};

/** `dark.css` as a spec, on the same terms as `OCULUS_LIGHT`. */
export const OCULUS_DARK: ThemeSpec = {
  id: "oculus-dark",
  name: "Oculus Dark",
  mode: "dark",
  builtin: true,
  palette: {
    ground: "#171717",
    card: "#212121",
    popover: "#2a2a2a",
    foreground: "#ececec",
    mutedForeground: "#a1a1a1",
    border: "#333333",
    accent: "#8b93e8",
    destructive: "#ef4444",
    success: "#22c55e",
    warning: "#f59e0b",
  },
  syntax: {
    keyword: "#f07a95",
    string: "#93c99f",
    comment: "#909090",
    number: "#e0a35e",
    function: "#b9a4f2",
    type: "#6cc4d4",
    property: "#8fb6f0",
    operator: "#b0b0b0",
    meta: "#d0b06e",
  },
};
