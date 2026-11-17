import { FolderSimple } from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { subjectTabIcon } from "@/components/subjects/nav/tabs";
import { useSubjects } from "@/hooks/data/useSubjects";
import { useInSidePanel } from "@/components/tabs/PaneHeader";
import { BreadcrumbItems, type BreadcrumbItem } from "@/components/ui/Breadcrumb";
import { filesFolderHref } from "@/lib/drive/routes";
import { parentRel } from "@/lib/drive/paths";
import { courseRelOf } from "@/lib/files/libraryLayout";
import { displayCode } from "@/lib/format/format";

/** A subject tab as a crumb: path segment under the subject, and its label. */
export interface CrumbTab {
  to: string;
  label: string;
}

export const LECTURES_TAB: CrumbTab = { to: "lectures", label: "Lectures" };

/** The folder a library file sits in, inside its subject (`Course/Files`),
 *  for `SubjectCrumbs`' `folder`; `""` at the subject's root. */
export const fileCrumbFolder = (file: { relative_path: string }): string =>
  parentRel(courseRelOf(file.relative_path));

/**
 * The subject, then a tab or a file's folders, before a page's leaf title.
 * `folder` is a file's folder inside the subject: Files, then one crumb per
 * folder, each opening that folder in the subject's Files tab.
 */
export function SubjectCrumbs({ subjectId, tab, folder }: {
  subjectId: number;
  tab?: CrumbTab | null;
  folder?: string;
}) {
  const { subjects } = useSubjects();
  const subject = subjects.find((s) => s.id === subjectId);
  const inSide = useInSidePanel();
  if (!subject) return null;
  const items: BreadcrumbItem[] = [{
    label: displayCode(subject.code), href: `/subjects/${subject.id}`,
    icon: <SubjectIcon code={subject.code} size={12} />,
  }];
  if (tab) {
    const Icon = subjectTabIcon(tab.to);
    items.push({ label: tab.label, href: `/subjects/${subject.id}/${tab.to}`, icon: <Icon size={12} /> });
  }
  if (folder != null) {
    const Files = subjectTabIcon("files");
    items.push({ label: "Files", href: filesFolderHref(subject.id, ""), icon: <Files size={12} /> });
    let rel = "";
    for (const name of folder.split("/").filter(Boolean)) {
      rel = rel ? `${rel}/${name}` : name;
      items.push({ label: name, href: filesFolderHref(subject.id, rel), icon: <FolderSimple size={12} /> });
    }
  }
  return <BreadcrumbItems items={items} visible={inSide ? 0 : 2} trailing />;
}
