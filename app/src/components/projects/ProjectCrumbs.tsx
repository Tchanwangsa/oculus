import { navigateActive } from "@/lib/tabRouters";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode } from "@/lib/format";
import type { DbProject } from "@/lib/projects";

/**
 * Where a project sits, as links rather than as decoration.
 *
 * Both the project page and a task page opened the same trail — the subject,
 * then a slash, then the thing you are looking at — and neither segment went
 * anywhere, so a project reached from Home or from a task had no way back to
 * the list it belongs to except the sidebar. The trail now starts at
 * **Projects** and the subject leads to that subject's own projects tab, which
 * are the two lists this page could have been opened from.
 *
 * A **Fragment, not a wrapper**: it drops into each page's existing crumb row
 * and inherits that row's gap, so the project page's toolbar (`gap-2.5`) and
 * the task page's tighter line (`gap-1.5`) each keep the spacing they had.
 * The trailing separator is part of the trail because what follows it is the
 * page's own leaf — an editable title on one, a link on the other — and those
 * belong to the pages, not here.
 *
 * Buttons with a `data-tab-href`, not `Link`s, like `SubjectCrumbs`: the plain
 * click goes through the shell's departure rules at `navigateActive`, and the
 * attribute is what gives the ⌘-click its own tab
 * (`app/src/lib/newTabClicks.ts`). A `Link` gave ⌘-click to the anchor's
 * default, which reloaded the whole webview at the crumb's path.
 *
 * Personal stays plain text. Its only list is the one `Projects` already
 * points at, and a second crumb to the same place is not a trail.
 */
export function ProjectCrumbs({ project }: { project: DbProject }) {
  return (
    <>
      <button
        type="button"
        data-tab-href="/projects"
        onClick={() => navigateActive("/projects")}
        className="shrink-0 cursor-pointer transition-colors hover:text-foreground"
      >
        Projects
      </button>
      <Separator />
      {project.subject_id != null && project.subject_code ? (
        <button
          type="button"
          data-tab-href={`/subjects/${project.subject_id}/projects`}
          onClick={() => navigateActive(`/subjects/${project.subject_id}/projects`)}
          className="flex shrink-0 cursor-pointer items-center gap-1.5 transition-colors hover:text-foreground"
        >
          <SubjectIcon code={project.subject_code} size={12} />
          {displayCode(project.subject_code)}
        </button>
      ) : (
        <span className="shrink-0">Personal</span>
      )}
      <Separator />
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
