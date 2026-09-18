import { navigateActive } from "@/lib/tabRouters";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useSubjects } from "@/hooks/useSubjects";
import { displayCode } from "@/lib/format";

/** A subject tab, as a crumb: the path segment under the subject, and what
 *  the tab strip calls it. */
export interface CrumbTab {
  to: string;
  label: string;
}

export const LECTURES_TAB: CrumbTab = { to: "lectures", label: "Lectures" };

/**
 * Which tab lists a file of this category — the list the page was opened
 * from, and the one place a reader would go looking for its siblings.
 *
 * The categories are Rust's (`category_from_path` in
 * `app/src-tauri/src/paths.rs`) and the tabs are `SubjectLayout`'s, so this
 * map is the seam between them; a category with no entry falls back to the
 * subject alone. `home` and `syllabus` are deliberately absent: they live on
 * Overview, which is exactly where the subject crumb already points, and a
 * second crumb to the same place is not a trail.
 */
const FILE_TAB: Record<string, CrumbTab> = {
  page: { to: "modules", label: "Modules" },
  module: { to: "modules", label: "Modules" },
  file: { to: "downloads", label: "Downloads" },
  upload: { to: "uploads", label: "Uploads" },
  announcement: { to: "announcements", label: "Announcements" },
  assignment: { to: "assignments", label: "Assignments" },
  quiz: { to: "assignments", label: "Assignments" },
  ed: { to: "discussion", label: "Discussion" },
};

export const fileCrumbTab = (category: string | null): CrumbTab | null =>
  (category && FILE_TAB[category]) || null;

/**
 * Where a full page sits inside its subject, as links rather than as
 * decoration — `ProjectCrumbs` for the other half of the app.
 *
 * A file and a lecture promoted out of the peek land *outside*
 * `SubjectLayout` on purpose: a full-page document takes the whole content
 * area, Notion-style, with no subject chrome. The cost was that the chrome
 * carried the only way back — open a lecture from Home or from the ⌘K palette
 * and the subject it belongs to was neither named nor reachable except
 * through the sidebar. This is that trail, and only that: the subject, then
 * the tab whose list holds this thing.
 *
 * A **Fragment, not a wrapper**, for the same reason as `ProjectCrumbs` — it
 * drops into the page's own crumb row, inherits its gap, and ends with the
 * separator the page's leaf follows.
 *
 * Buttons with a `data-tab-href`, not `Link`s, and the difference is not
 * cosmetic. The attribute gives ⌘-click a tab of its own
 * (`app/src/lib/newTabClicks.ts`); the plain click goes through
 * `navigateActive`. A `Link` goes straight to the pane's own router, which skips the
 * one door the app's departure rules live behind: a crumb clicked out of a
 * playing lecture stranded it off screen — still running, no player anywhere,
 * no prompt — and a crumb clicked out of a page with a peek open carried that
 * peek to a page it had nothing to do with. The shell has asked these
 * questions at `navigateActive` since tabs got their own routers; the crumb
 * row is inside a pane, which is the only reason it was ever missed.
 *
 * It resolves the subject itself rather than taking one, because its two
 * callers each have an id and no subject: they are the pages that skipped the
 * layout that would have loaded it.
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

  // Nothing rather than a placeholder: the row's leaf is already the title,
  // and a crumb that resolves a moment later reads as the page arriving.
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
