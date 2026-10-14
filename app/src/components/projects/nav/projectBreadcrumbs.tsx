import { User } from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import type { BreadcrumbItem } from "@/components/ui/Breadcrumb";
import { displayCode } from "@/lib/format/format";
import { tasksHref } from "../tasks/taskViews";
import type { DbProject } from "@/lib/planning/projects";

export function projectParents(project: DbProject): BreadcrumbItem[] {
  return [
    { label: "Projects", href: "/projects" },
    project.subject_id != null && project.subject_code
      ? { label: displayCode(project.subject_code), href: tasksHref("all", null, project.subject_id), icon: <SubjectIcon code={project.subject_code} size={12} /> }
      : { label: "Personal", href: tasksHref("all", null, "personal"), icon: <User size={12} /> },
  ];
}
