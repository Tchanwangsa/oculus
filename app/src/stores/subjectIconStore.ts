import { create } from "zustand";
import { persist } from "zustand/middleware";
import { displayCode } from "@/lib/format";

/**
 * Per-subject icon customisation: a Phosphor icon name and/or a colour.
 * Keyed by course code ("COMP30026"), not the full term-suffixed code, so the
 * same course keeps its icon across semesters.
 */
export interface SubjectIconPref {
  /** Name from the icon catalog. Absent → the default dot. */
  icon?: string;
  /** Hex colour. Absent → the hashed course colour. */
  color?: string;
}

interface SubjectIconState {
  prefs: Record<string, SubjectIconPref>;
  setPref: (code: string, patch: Partial<SubjectIconPref>) => void;
  clearPref: (code: string) => void;
}

export const iconKey = (code: string) => displayCode(code);

const AUTO_PALETTE = [
  "#8b93e8",
  "#6ba5d7",
  "#a78bfa",
  "#d79b6b",
  "#7bbf8e",
  "#d78bb0",
  "#c9b26b",
];

/** Deterministic fallback colour hashed from the course code. */
export function courseColor(code: string): string {
  let h = 0;
  for (let i = 0; i < code.length; i++) h = code.charCodeAt(i) + ((h << 5) - h);
  return AUTO_PALETTE[Math.abs(h) % AUTO_PALETTE.length];
}

export const useSubjectIconStore = create<SubjectIconState>()(
  persist(
    (set) => ({
      prefs: {},
      setPref: (code, patch) =>
        set((s) => {
          const key = iconKey(code);
          const merged: SubjectIconPref = { ...s.prefs[key], ...patch };
          if (merged.icon === undefined) delete merged.icon;
          if (merged.color === undefined) delete merged.color;
          const next = { ...s.prefs };
          if (Object.keys(merged).length === 0) delete next[key];
          else next[key] = merged;
          return { prefs: next };
        }),
      clearPref: (code) =>
        set((s) => {
          const next = { ...s.prefs };
          delete next[iconKey(code)];
          return { prefs: next };
        }),
    }),
    { name: "oculus-subject-icons" },
  ),
);

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
