import type { ReactNode } from "react";
import { IconPicker } from "@/components/ui/icon/IconPicker";
import { useProjectIconStore } from "@/stores/planning/projectIconStore";
import { useProjectIdentity, type ProjectIdentity } from "./ProjectIcon";

export function ProjectIconPicker({ project, children }: { project: ProjectIdentity; children: ReactNode }) {
  const { pref, defaultColor } = useProjectIdentity(project);
  const setPref = useProjectIconStore(s => s.setPref);
  const clearPref = useProjectIconStore(s => s.clearPref);
  return <IconPicker value={pref} defaultName="Cube" defaultColor={defaultColor} weight="regular"
    onChange={patch => setPref(project.id, patch)} onReset={() => clearPref(project.id)}>{children}</IconPicker>;
}
