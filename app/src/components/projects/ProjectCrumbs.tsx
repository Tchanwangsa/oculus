import { navigateActive } from "@/lib/tabRouters";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode } from "@/lib/format";
import type { DbProject } from "@/lib/projects";

/**
 * Projects / subject / — the trail before a project or task page's own leaf.
 * A Fragment so it inherits the host row's gap. Buttons with `data-tab-href`,
 * not `Link`s: a `Link`'s ⌘-click reloads the whole webview at that path.
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
