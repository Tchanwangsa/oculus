import { Link } from "react-router-dom";
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
 * Personal stays plain text. Its only list is the one `Projects` already
 * points at, and a second crumb to the same place is not a trail.
 */
export function ProjectCrumbs({ project }: { project: DbProject }) {
  return (
    <>
      <Link to="/projects" className="shrink-0 transition-colors hover:text-foreground">
        Projects
      </Link>
      <Separator />
      {project.subject_id != null && project.subject_code ? (
        <Link
          to={`/subjects/${project.subject_id}/projects`}
          className="flex shrink-0 items-center gap-1.5 transition-colors hover:text-foreground"
        >
          <SubjectIcon code={project.subject_code} size={12} />
          {displayCode(project.subject_code)}
        </Link>
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
