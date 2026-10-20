import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { subjectTabIcon } from "@/components/subjects/nav/tabs";
import { useSubjects } from "@/hooks/data/useSubjects";
import { useInSidePanel } from "@/components/tabs/PaneHeader";
import { BreadcrumbItems, type BreadcrumbItem } from "@/components/ui/Breadcrumb";
import { displayCode } from "@/lib/format/format";
import { isVideoFile } from "@/lib/files/fileTypes";

/** A subject tab as a crumb: path segment under the subject, and its label. */
export interface CrumbTab {
  to: string;
  label: string;
}

export const LECTURES_TAB: CrumbTab = { to: "lectures", label: "Lectures" };

/**
 * The tab that lists a file of each category — the seam between Rust's
 * categories (`category_from_path` in `app/src-tauri/src/library/paths/categories.rs`) and
 * `SubjectLayout`'s tabs. `home`/`syllabus` are absent: they live on Overview,
 * where the subject crumb already points. Unmapped falls back to the subject.
 */
const FILE_TAB: Record<string, CrumbTab> = {
  page: { to: "modules", label: "Modules" },
  module: { to: "modules", label: "Modules" },
  // Sub-tabs of one Files tab: the crumb says "Files", the path picks the sub-tab.
  file: { to: "files/downloads", label: "Files" },
  upload: { to: "files/uploads", label: "Files" },
  document: { to: "files/documents", label: "Files" },
  announcement: { to: "announcements", label: "Announcements" },
  assignment: { to: "assignments", label: "Assignments" },
  quiz: { to: "assignments", label: "Assignments" },
  ed: { to: "discussion", label: "Discussion" },
  lessons: { to: "lessons", label: "Lessons" },
};

/** Downloaded videos are listed under Media, not Downloads (`MediaPage`). */
const MEDIA_TAB: CrumbTab = { to: "files/media", label: "Files" };

export const fileCrumbTab = (file: { category: string | null; filename: string }): CrumbTab | null =>
  file.category === "file" && isVideoFile(file.filename)
    ? MEDIA_TAB
    : (file.category && FILE_TAB[file.category]) || null;

/** The subject and category before a file or lecture's leaf title. */
export function SubjectCrumbs({ subjectId, tab }: { subjectId: number; tab?: CrumbTab | null }) {
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
  return <BreadcrumbItems items={items} visible={inSide ? 0 : 2} trailing />;
}
