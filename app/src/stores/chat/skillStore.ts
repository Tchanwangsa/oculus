import { create } from "zustand";
import { harnessSkills, type Skill } from "@/lib/harness/skills";

/**
 * The installed skills every composer offers on `/` (`harness_skills`). Read
 * when a composer mounts and again each time its `/` menu opens, so a skill
 * installed from a terminal shows up without a relaunch. `skills` is null
 * until the first read lands; a failed read is an empty catalogue.
 */
export const useSkillStore = create<{
  skills: Skill[] | null;
  /** The same names as a set, for drawing a stored draft's `/name` as a chip. */
  names: ReadonlySet<string> | null;
  refresh: () => void;
}>((set, get) => {
  let inFlight = false;
  return {
    skills: null,
    names: null,
    refresh: () => {
      if (inFlight) return;
      inFlight = true;
      harnessSkills()
        .catch((e) => {
          console.error("harness skills failed", e);
          return get().skills ?? [];
        })
        .then((skills) => {
          const prev = get().skills;
          const same =
            prev != null &&
            prev.length === skills.length &&
            prev.every((s, i) => s.path === skills[i].path && s.description === skills[i].description);
          if (!same) set({ skills, names: new Set(skills.map((s) => s.name)) });
        })
        .finally(() => {
          inFlight = false;
        });
    },
  };
});
