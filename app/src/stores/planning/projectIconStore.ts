import { create } from "zustand";
import { persist } from "zustand/middleware";
import { ICON_COLORS, mergeIconPreference, type IconPreference } from "@/lib/ui/iconPreferences";
import { courseColor } from "./subjectIconStore";

interface ProjectIconState {
  prefs: Record<string, IconPreference>;
  setPref: (id: number, patch: Partial<IconPreference>) => void;
  clearPref: (id: number) => void;
}

/** Unset colours follow the subject's current colour; Personal defaults to grey. */
export function projectDefaultColor(code: string | null | undefined, subjectColor?: string): string {
  return code ? subjectColor ?? courseColor(code) : ICON_COLORS[0].value;
}

export const useProjectIconStore = create<ProjectIconState>()(persist(set => ({
  prefs: {},
  setPref: (id, patch) => set(state => {
    const pref = mergeIconPreference(state.prefs[id], patch);
    const prefs = { ...state.prefs };
    if (pref) prefs[id] = pref;
    else delete prefs[id];
    return { prefs };
  }),
  clearPref: id => set(state => {
    const prefs = { ...state.prefs };
    delete prefs[id];
    return { prefs };
  }),
}), { name: "oculus-project-icons" }));
