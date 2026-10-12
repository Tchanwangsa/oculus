import type { ThemeSpec } from "./types";

// Published palettes, with the app's structure: the ground one step off the
// card. A colour that reads too faintly on the card or on `surface-raised`
// (where code sits) is darkened along its own hue; the test holds the floor.

export const PAPER: ThemeSpec = {
  id: "paper",
  name: "Paper",
  mode: "light",
  palette: {
    ground: "#f2ede3",
    card: "#fbf8f2",
    popover: "#fdfbf7",
    foreground: "#2b241c",
    mutedForeground: "#6f6456",
    border: "#e2dacb",
    accent: "#a9573b",
  },
  syntax: {
    keyword: "#a1413a",
    string: "#4d7038",
    comment: "#857a6b",
    number: "#9a5a12",
    function: "#6c4f8f",
    type: "#2f6b72",
    property: "#3f5f86",
    operator: "#6a5f52",
    meta: "#86642a",
  },
};

export const GITHUB_LIGHT: ThemeSpec = {
  id: "github-light",
  name: "GitHub Light",
  mode: "light",
  palette: {
    ground: "#f6f8fa",
    card: "#ffffff",
    popover: "#ffffff",
    foreground: "#1f2328",
    mutedForeground: "#59636e",
    border: "#d1d9e0",
    accent: "#0969da",
    destructive: "#d1242f",
    success: "#1a7f37",
    warning: "#9a6700",
  },
  syntax: {
    keyword: "#cf222e",
    string: "#0a3069",
    comment: "#6e7781",
    number: "#0550ae",
    function: "#8250df",
    type: "#953800",
    property: "#0550ae",
    operator: "#1f2328",
    meta: "#116329",
  },
};

export const SOLARIZED_LIGHT: ThemeSpec = {
  id: "solarized-light",
  name: "Solarized Light",
  mode: "light",
  palette: {
    ground: "#eee8d5",
    card: "#fdf6e3",
    popover: "#fdf6e3",
    foreground: "#073642",
    mutedForeground: "#586e75",
    border: "#e0d9c3",
    accent: "#268bd2",
    destructive: "#dc322f",
    success: "#7e8b00",
    warning: "#a77b00",
  },
  syntax: {
    keyword: "#7e8b00",
    string: "#17938a",
    comment: "#7d8585",
    number: "#d33682",
    function: "#2489d0",
    type: "#a77b00",
    property: "#6c71c4",
    operator: "#586e75",
    meta: "#cb4b16",
  },
};

export const CATPPUCCIN_LATTE: ThemeSpec = {
  id: "catppuccin-latte",
  name: "Catppuccin Latte",
  mode: "light",
  palette: {
    ground: "#e6e9ef",
    card: "#eff1f5",
    popover: "#eff1f5",
    foreground: "#4c4f69",
    mutedForeground: "#5c5f77",
    border: "#ccd0da",
    accent: "#8839ef",
    destructive: "#d20f39",
    success: "#34941f",
    warning: "#bf6e00",
  },
  syntax: {
    keyword: "#8839ef",
    string: "#34941f",
    comment: "#7c7f93",
    number: "#e64c00",
    function: "#1e66f5",
    type: "#bf6e00",
    property: "#6975eb",
    operator: "#0089c9",
    meta: "#ca56ab",
  },
};

export const ROSE_PINE_DAWN: ThemeSpec = {
  id: "rose-pine-dawn",
  name: "Rosé Pine Dawn",
  mode: "light",
  palette: {
    ground: "#faf4ed",
    card: "#fffaf3",
    popover: "#fffaf3",
    foreground: "#575279",
    mutedForeground: "#706c8a",
    border: "#dfdad9",
    accent: "#286983",
    destructive: "#b4637a",
    success: "#286983",
    warning: "#cc7f0d",
  },
  syntax: {
    keyword: "#286983",
    string: "#c27500",
    comment: "#898695",
    number: "#b4637a",
    function: "#c57075",
    type: "#4f8d98",
    property: "#907aa9",
    operator: "#797593",
    meta: "#907aa9",
  },
};

export const LIGHT_SAMPLES = [PAPER, GITHUB_LIGHT, SOLARIZED_LIGHT, CATPPUCCIN_LATTE, ROSE_PINE_DAWN];
