import { ProjectIcon } from "../icons/ProjectIcon";
import { ProjectIconPicker } from "../icons/ProjectIconPicker";
import { useMemo } from "react";
import { DetailBody } from "@/components/projects/detail/DetailBody";
import { DetailTitle } from "@/components/projects/detail/DetailTitle";
import { projectHref } from "@/components/projects/nav/projectHref";
import type { DbProject, UpdateProjectInput } from "@/lib/planning/projects";
import { boardProgress, type TaskNode } from "../tasks/taskTree";
import { ProjectProperties } from "./ProjectProperties";
import { Heading } from "./overviewParts";
import { UpcomingTasks } from "./UpcomingTasks";

/** A project reads as a task document: title, properties, then its rich About. */
export function ProjectOverview({ project, nodes, onPatch, onRename }: {
  project: DbProject;
  nodes: TaskNode[];
  onPatch: (patch: UpdateProjectInput) => void;
  onRename: (name: string) => void;
}) {
  const progress = useMemo(() => boardProgress(nodes), [nodes]);
  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-3xl px-6 pt-8 pb-16">
        <div className="flex items-start gap-3">
          <ProjectIconPicker project={project}>
            <button type="button" aria-label={`Change icon for ${project.name}`} className="mt-0.5 flex size-9 shrink-0 cursor-pointer items-center justify-center rounded-lg outline-none transition-colors hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50">
              <ProjectIcon project={project} size={30} />
            </button>
          </ProjectIconPicker>
          <div className="min-w-0 flex-1"><DetailTitle key={`title-${project.id}`} title={project.name} label="Project name"
          href={(name) => projectHref({ id: project.id, name })} onRename={onRename} /></div>
        </div>
        <ProjectProperties project={project} progress={progress} onPatch={onPatch} />
        <section className="mt-8">
          <Heading>About</Heading>
          <DetailBody key={`about-${project.id}`} text={project.brief} subjectId={project.subject_id}
            kind="project" className="-mx-2" onSave={(brief) => onPatch({ brief })} />
        </section>
        <div className="mt-8"><UpcomingTasks project={project} nodes={nodes} /></div>
      </div>
    </div>
  );
}
