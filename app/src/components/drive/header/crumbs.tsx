import { FolderSimple, LockSimple, type Icon } from "@phosphor-icons/react";
import type { BreadcrumbItem } from "@/components/ui/Breadcrumb";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import type { Subject } from "@/lib/db";
import { displayCode } from "@/lib/format/format";
import { subjectForFolder } from "@/lib/drive/paths";

/**
 * The trail from the browser's root to `folder`: the root's own label, then
 * one crumb per folder, a subject's by its code and icon. The last crumb is
 * the folder on screen, locked when the student can't add to it.
 */
export function driveCrumbs({ root, folder, rootLabel, rootIcon: RootIcon, subjects, folderHref, locked }: {
  root: string;
  folder: string;
  rootLabel: string;
  rootIcon: Icon;
  subjects: readonly Subject[];
  folderHref: (rel: string) => string;
  locked: boolean;
}): BreadcrumbItem[] {
  const items: BreadcrumbItem[] = [
    { label: rootLabel, href: folder === root ? undefined : folderHref(root), icon: <RootIcon size={14} /> },
  ];
  const below = folder === root ? [] : folder.slice(root ? root.length + 1 : 0).split("/");
  let rel = root;
  below.forEach((name, index) => {
    rel = rel ? `${rel}/${name}` : name;
    const last = index === below.length - 1;
    // At the Drive root the first two segments are the subjects' folder and
    // a subject; the subjects' folder itself is not a place to go.
    if (!root && index === 0) return;
    const subject = !root && index === 1 ? subjectForFolder(name, subjects) : undefined;
    const Glyph = last && locked ? LockSimple : FolderSimple;
    items.push({
      label: subject ? displayCode(subject.code) : name,
      href: last ? undefined : folderHref(rel),
      icon: subject ? <SubjectIcon code={subject.code} size={14} /> : <Glyph size={14} />,
    });
  });
  return items;
}
