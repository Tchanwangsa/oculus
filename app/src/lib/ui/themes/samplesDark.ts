import type { ThemeSpec } from "./types";

// Published palettes, with the app's structure: the ground one step off the
// card. A colour that reads too faintly on the card or on `surface-raised`
// (where code sits) is lightened along its own hue; the test holds the floor.

export const MIDNIGHT: ThemeSpec = {
  id: "midnight",
  name: "Midnight",
  mode: "dark",
  palette: {
    ground: "#000000",
    card: "#0d0d0d",
    popover: "#171717",
    foreground: "#ececec",
    mutedForeground: "#9a9a9a",
    border: "#232323",
    accent: "#8b93e8",
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

export const NORD: ThemeSpec = {
  id: "nord",
  name: "Nord",
  mode: "dark",
  palette: {
    ground: "#292e39",
    card: "#2e3440",
    popover: "#3b4252",
    foreground: "#eceff4",
    mutedForeground: "#a3abb9",
    border: "#3b4252",
    accent: "#88c0d0",
    destructive: "#bf616a",
    success: "#a3be8c",
    warning: "#ebcb8b",
  },
  syntax: {
    keyword: "#81a1c1",
    string: "#a3be8c",
    comment: "#8590a8",
    number: "#b48ead",
    function: "#88c0d0",
    type: "#8fbcbb",
    property: "#d8dee9",
    operator: "#81a1c1",
    meta: "#d08770",
  },
};

export const TOKYO_NIGHT: ThemeSpec = {
  id: "tokyo-night",
  name: "Tokyo Night",
  mode: "dark",
  palette: {
    ground: "#16161e",
    card: "#1a1b26",
    popover: "#1f2335",
    foreground: "#c0caf5",
    mutedForeground: "#7c84ad",
    border: "#292e42",
    accent: "#7aa2f7",
    destructive: "#f7768e",
    success: "#9ece6a",
    warning: "#e0af68",
  },
  syntax: {
    keyword: "#bb9af7",
    string: "#9ece6a",
    comment: "#6b7394",
    number: "#ff9e64",
    function: "#7aa2f7",
    type: "#2ac3de",
    property: "#73daca",
    operator: "#89ddff",
    meta: "#e0af68",
  },
};

export const CATPPUCCIN_MOCHA: ThemeSpec = {
  id: "catppuccin-mocha",
  name: "Catppuccin Mocha",
  mode: "dark",
  palette: {
    ground: "#181825",
    card: "#1e1e2e",
    popover: "#313244",
    foreground: "#cdd6f4",
    mutedForeground: "#9399b2",
    border: "#313244",
    accent: "#cba6f7",
    destructive: "#f38ba8",
    success: "#a6e3a1",
    warning: "#f9e2af",
  },
  syntax: {
    keyword: "#cba6f7",
    string: "#a6e3a1",
    comment: "#9399b2",
    number: "#fab387",
    function: "#89b4fa",
    type: "#f9e2af",
    property: "#b4befe",
    operator: "#89dceb",
    meta: "#f5c2e7",
  },
};

export const ROSE_PINE: ThemeSpec = {
  id: "rose-pine",
  name: "Rosé Pine",
  mode: "dark",
  palette: {
    ground: "#191724",
    card: "#1f1d2e",
    popover: "#26233a",
    foreground: "#e0def4",
    mutedForeground: "#908caa",
    border: "#2a273f",
    accent: "#c4a7e7",
    destructive: "#eb6f92",
    success: "#9ccfd8",
    warning: "#f6c177",
  },
  syntax: {
    keyword: "#4a85a3",
    string: "#f6c177",
    comment: "#767296",
    number: "#eb6f92",
    function: "#ebbcba",
    type: "#9ccfd8",
    property: "#c4a7e7",
    operator: "#908caa",
    meta: "#c4a7e7",
  },
};

export const DRACULA: ThemeSpec = {
  id: "dracula",
  name: "Dracula",
  mode: "dark",
  palette: {
    ground: "#21222c",
    card: "#282a36",
    popover: "#343746",
    foreground: "#f8f8f2",
    mutedForeground: "#8a93b8",
    border: "#3a3c4e",
    accent: "#bd93f9",
    destructive: "#ff5555",
    success: "#50fa7b",
    warning: "#ffb86c",
  },
  syntax: {
    keyword: "#ff79c6",
    string: "#f1fa8c",
    comment: "#7a86b0",
    number: "#bd93f9",
    function: "#50fa7b",
    type: "#8be9fd",
    property: "#f8f8f2",
    operator: "#ff79c6",
    meta: "#ffb86c",
  },
};

export const NIGHT_OWL: ThemeSpec = {
  id: "night-owl",
  name: "Night Owl",
  mode: "dark",
  palette: {
    ground: "#010e1a",
    card: "#011627",
    popover: "#0b2942",
    foreground: "#d6deeb",
    mutedForeground: "#6e86a0",
    border: "#122d42",
    accent: "#82aaff",
    destructive: "#ef5350",
    success: "#22da6e",
    warning: "#ffcb8b",
  },
  syntax: {
    keyword: "#c792ea",
    string: "#ecc48d",
    comment: "#637777",
    number: "#f78c6c",
    function: "#82aaff",
    type: "#ffcb8b",
    property: "#7fdbca",
    operator: "#c792ea",
    meta: "#addb67",
  },
};

export const DARK_SAMPLES = [MIDNIGHT, NORD, TOKYO_NIGHT, CATPPUCCIN_MOCHA, ROSE_PINE, DRACULA, NIGHT_OWL];
