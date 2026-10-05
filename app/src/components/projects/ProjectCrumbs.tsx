import { navigateActive } from "@/lib/tabRouters";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { TrailMore, useInSidePanel, useTrailCollapsed } from "@/components/tabs/PaneHeader";
import { displayCode } from "@/lib/format";
import type { DbProject } from "@/lib/projects";

/**
 * Projects / subject / — the trail before a project or task page's own leaf.
 * A Fragment so it inherits the host row's gap. Buttons with `data-tab-href`,
 * not `Link`s: a `Link`'s ⌘-click reloads the whole webview at that path.
 *
 * In the side panel the subject drops its icon (the switcher shows the
 * item's), and folded (`PaneHeader.tsx`) "Projects" sits behind a "…" in
 * place — the subject too with `foldSubject`, when the leaf is the project.
 */
export function ProjectCrumbs({
  project,
  foldSubject,
}: {
  project: DbProject;
  foldSubject?: boolean;
}) {
  const inSide = useInSidePanel();
  const folded = useTrailCollapsed();

  const subject =
    project.subject_id != null && project.subject_code ? (
      <button
        type="button"
        data-tab-href={`/subjects/${project.subject_id}/projects`}
        onClick={() => navigateActive(`/subjects/${project.subject_id}/projects`)}
        className="flex shrink-0 cursor-pointer items-center gap-1.5 transition-colors hover:text-foreground"
      >
        {!inSide && <SubjectIcon code={project.subject_code} size={12} />}
        {displayCode(project.subject_code)}
      </button>
    ) : (
      <span className="shrink-0">Personal</span>
    );

  if (folded) {
    return (
      <>
        <TrailMore />
        <Separator />
        {!foldSubject && (
          <>
            {subject}
            <Separator />
          </>
        )}
      </>
    );
  }

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
      {subject}
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
