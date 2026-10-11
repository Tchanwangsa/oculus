import { useMemo } from "react";
import { Badge } from "@/components/ui/badge";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode } from "@/lib/format/format";
import type { DbProject, UpdateProjectInput } from "@/lib/planning/projects";
import { DateTimeField } from "../fields/DateTimeField";
import { EventLink } from "./EventLink";
import { TagEditor } from "../fields/TagEditor";
import { ProgressMeter } from "../tasks/TaskMarks";
import { boardProgress, type TaskNode } from "../tasks/taskTree";
import { Brief } from "./Brief";
import { Heading, Row } from "./overviewParts";
import { UpcomingTasks } from "./UpcomingTasks";

/** A project's description, properties and next few dated tasks. */

export function ProjectOverview({
  project,
  nodes,
  onPatch,
}: {
  project: DbProject;
  nodes: TaskNode[];
  onPatch: (patch: UpdateProjectInput) => void;
}) {
  const progress = useMemo(() => boardProgress(nodes), [nodes]);

  return (
    <div className="page-scroll">
      <div className="mx-auto flex max-w-3xl flex-col gap-8 px-6 py-6">
        <section>
          <Heading>About</Heading>
          <Brief project={project} onSave={(brief) => onPatch({ brief })} />
        </section>

        <section>
          <Heading>Properties</Heading>
          <div className="flex flex-col gap-0.5">
            <Row label="Subject">
              {/* Read-only: re-scoping moves every task on the calendar. */}
              <span className="flex min-w-0 items-center gap-1.5 text-xs text-foreground">
                {project.subject_code ? (
                  <>
                    <SubjectIcon code={project.subject_code} size={13} />
                    {displayCode(project.subject_code)}
                  </>
                ) : (
                  "Personal"
                )}
              </span>
            </Row>

            <Row label="Status">
              <Badge variant="secondary" className="text-[11px]">
                {project.status === "archived" ? "Archived" : "Active"}
              </Badge>
            </Row>

            <Row label="Starts">
              <DateTimeField
                value={project.starts_at}
                defaultTime="start"
                onCommit={(startsAt) => onPatch({ startsAt })}
              />
            </Row>

            <Row label="Due">
              <DateTimeField
                value={project.due_at}
                defaultTime="end"
                onCommit={(dueAt) => onPatch({ dueAt })}
              />
            </Row>

            <Row label="Tags">
              <TagEditor tags={project.tags} onChange={(tags) => onPatch({ tags })} />
            </Row>

            <Row label="Event">
              <EventLink project={project} onPick={(eventId) => onPatch({ eventId })} />
            </Row>

            <Row label="Progress">
              {progress.total === 0 ? (
                // No zero bar — it reads as stalled (as `ProjectProgress`).
                <span className="text-[11px] text-muted-foreground/60">Nothing planned yet</span>
              ) : (
                <ProgressMeter done={progress.done} total={progress.total} />
              )}
            </Row>
          </div>
        </section>

        <UpcomingTasks project={project} nodes={nodes} />
      </div>
    </div>
  );
}
