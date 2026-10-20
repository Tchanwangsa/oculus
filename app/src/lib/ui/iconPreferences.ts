/** Appearance overrides; omitted fields keep following their automatic defaults. */
export interface IconPreference {
  icon?: string;
  color?: string;
}

/** Clearing one override preserves the other; an empty preference stores nothing. */
export function mergeIconPreference(current: IconPreference | undefined, patch: Partial<IconPreference>): IconPreference | undefined {
  const next = { ...current, ...patch };
  if (next.icon === undefined) delete next.icon;
  if (next.color === undefined) delete next.color;
  return Object.keys(next).length ? next : undefined;
}

/** The colour picker's swatches. Mid-value so they read on both themes. */
export const ICON_COLORS = [
  { name: "Grey", value: "#8f98a0" },
  { name: "Red", value: "#e5484d" },
  { name: "Orange", value: "#e8823a" },
  { name: "Amber", value: "#d6a243" },
  { name: "Green", value: "#46a758" },
  { name: "Teal", value: "#12a594" },
  { name: "Blue", value: "#4c8fd6" },
  { name: "Indigo", value: "#5e6ad2" },
  { name: "Purple", value: "#8e4ec6" },
  { name: "Pink", value: "#d6409f" },
] as const;
