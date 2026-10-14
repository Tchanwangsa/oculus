import { mergeIconPreference, type IconPreference } from "@/lib/ui/iconPreferences";
import { create } from "zustand";
import { persist } from "zustand/middleware";
import { displayCode } from "@/lib/format/format";

interface SubjectIconState {
  prefs: Record<string, IconPreference>;
  setPref: (code: string, patch: Partial<IconPreference>) => void;
  clearPref: (code: string) => void;
}

/** A course keeps its appearance across terms sharing the display code. */
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
          const merged = mergeIconPreference(s.prefs[key], patch);
          const next = { ...s.prefs };
          if (!merged) delete next[key];
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
