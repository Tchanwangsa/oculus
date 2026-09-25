import { navigateActive } from "@/lib/tabRouters";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useSubjects } from "@/hooks/useSubjects";
import { displayCode } from "@/lib/format";

/** A subject tab as a crumb: path segment under the subject, and its label. */
export interface CrumbTab {
  to: string;
  label: string;
}

export const LECTURES_TAB: CrumbTab = { to: "lectures", label: "Lectures" };

/**
 * The tab that lists a file of each category — the seam between Rust's
 * categories (`category_from_path` in `app/src-tauri/src/paths.rs`) and
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
};

export const fileCrumbTab = (category: string | null): CrumbTab | null =>
  (category && FILE_TAB[category]) || null;

/**
 * The subject → tab trail for full pages outside `SubjectLayout` (a promoted
 * file or lecture); `ProjectCrumbs` is the other half. A Fragment, so it drops
 * into the page's own crumb row.
 *
 * Buttons with `data-tab-href`, not `Link`s: the click must go through
 * `navigateActive`, which runs the departure rules (a playing lecture, an open
 * peek) that the pane's own router skips; ⌘-click opens a new tab
 * (`app/src/lib/newTabClicks.ts`). Resolves the subject from the id itself.
 */
export function SubjectCrumbs({
  subjectId,
  tab,
}: {
  subjectId: number;
  tab?: CrumbTab | null;
}) {
  const { subjects } = useSubjects();
  const subject = subjects.find((s) => s.id === subjectId) ?? null;

  // Nothing rather than a placeholder; the leaf is already the title.
  if (!subject) return null;

  return (
    <>
      <button
        type="button"
        data-tab-href={`/subjects/${subject.id}`}
        onClick={() => navigateActive(`/subjects/${subject.id}`)}
        className="flex shrink-0 cursor-pointer items-center gap-1.5 transition-colors hover:text-foreground"
      >
        <SubjectIcon code={subject.code} size={12} />
        {displayCode(subject.code)}
      </button>
      <Separator />
      {tab && (
        <>
          <button
            type="button"
            data-tab-href={`/subjects/${subject.id}/${tab.to}`}
            onClick={() => navigateActive(`/subjects/${subject.id}/${tab.to}`)}
            className="shrink-0 cursor-pointer transition-colors hover:text-foreground"
          >
            {tab.label}
          </button>
          <Separator />
        </>
      )}
    </>
  );
}

function Separator() {
  return (
    <span aria-hidden className="shrink-0 text-border">
      /
    </span>
  );
}
