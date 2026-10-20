import { Cube } from "@phosphor-icons/react";
import { CatalogueIcon } from "@/components/ui/icon/CatalogueIcon";
import { projectDefaultColor, useProjectIconStore } from "@/stores/planning/projectIconStore";
import { useSubjectIconStore, iconKey } from "@/stores/planning/subjectIconStore";
import { useProjectsStore } from "@/stores/planning/projectsStore";

export interface ProjectIdentity {
  id: number;
  subject_code?: string | null;
}

export function useProjectIdentity(project: ProjectIdentity) {
  const storedCode = useProjectsStore(s => project.subject_code === undefined ? s.projects.find(p => p.id === project.id)?.subject_code : null);
  const code = project.subject_code === undefined ? storedCode : project.subject_code;
  const subjectColor = useSubjectIconStore(s => code ? s.prefs[iconKey(code)]?.color : undefined);
  const pref = useProjectIconStore(s => s.prefs[project.id]);
  return { pref, defaultColor: projectDefaultColor(code, subjectColor) };
}

/** The same project glyph in rows, breadcrumbs, tabs and destination pickers. */
export function ProjectIcon({ project, size = 14, className }: { project: ProjectIdentity; size?: number; className?: string }) {
  const { pref, defaultColor } = useProjectIdentity(project);
  return <CatalogueIcon name={pref?.icon} fallback={Cube} color={pref?.color ?? defaultColor} size={size} weight="regular" className={className} />;
}
