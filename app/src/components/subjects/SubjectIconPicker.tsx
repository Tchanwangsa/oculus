import type { ReactNode } from "react";
import { IconPicker } from "@/components/ui/icon/IconPicker";
import { courseColor, iconKey, useSubjectIconStore } from "@/stores/planning/subjectIconStore";

export function SubjectIconPicker({ code, children }: { code: string; children: ReactNode }) {
  const pref = useSubjectIconStore(s => s.prefs[iconKey(code)]);
  const setPref = useSubjectIconStore(s => s.setPref);
  const clearPref = useSubjectIconStore(s => s.clearPref);
  return <IconPicker value={pref} defaultName="Book" defaultColor={courseColor(code)}
    onChange={patch => setPref(code, patch)} onReset={() => clearPref(code)}>{children}</IconPicker>;
}
