import { useEffect, useMemo } from "react";
import {
  Outlet,
  useLocation,
  useOutletContext,
  useSearchParams,
} from "react-router-dom";
import { Cards, Exam, Lightbulb, Note, SquaresFour } from "@phosphor-icons/react";
import { useSideNavPanel } from "@/hooks/ui/useSideNavPanel";
import { useRevisionClock } from "@/hooks/revision/useRevisionClock";
import { SideNavShell } from "@/components/ui/layout/SideNavShell";
import {
  SideNav,
  SideNavHeading,
  SideNavLink,
} from "@/components/ui/layout/SideNav";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { NewCountBadge } from "@/components/NewCountBadge";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { SubjectSwitcherMenu } from "@/components/subjects/nav/SubjectSwitcher";
import { TopicsGroup } from "@/components/revision/nav/TopicsGroup";
import { useRevisionStore } from "@/stores/revision/revisionStore";
import { REVISION_UPDATED_EVENT } from "@/lib/revision/writes";
import {
  lastRevisionSubject,
  rememberRevisionSubject,
} from "@/lib/revision/lastSubject";
import {
  topicsForSubject,
  matchesTopic,
  inheritedTopic,
} from "@/lib/revision/topics";
import type {
  RevisionScope,
  RevisionTopicScope,
  RevisionTopic,
} from "@/lib/revision/types";
import { useSubjects } from "@/hooks/data/useSubjects";
import type { Subject } from "@/lib/db";
import { displayCode } from "@/lib/format/format";

/** The URL's subject, else the one Revision last showed, else the first. */
function pickSubject(
  wanted: string | null,
  subjects: Subject[],
  current: Subject[],
  past: Subject[],
): Subject | undefined {
  return (
    subjects.find((s) => String(s.id) === wanted || s.code === wanted) ??
    subjects.find((s) => s.id === lastRevisionSubject()) ??
    current[0] ??
    past[0]
  );
}

const options = {
  defaultWidth: 224,
  minWidth: 168,
  maxWidth: 360,
  collapsedWidth: 0,
  storageKey: "oculus-revision-nav-width",
};
type Panel = ReturnType<typeof useSideNavPanel>;
type RevisionData = Pick<
  ReturnType<typeof useRevisionStore.getState>,
  | "topics"
  | "notes"
  | "items"
  | "sets"
  | "artifacts"
  | "loading"
  | "error"
  | "reload"
> & {
  scope: RevisionScope;
  topicScope: RevisionTopicScope;
  creationTopic: RevisionTopic | undefined;
};
type RevisionView = RevisionData & { panel: Panel };

/** Data is shared; each pane's URL owns its subject scope. */
export default function RevisionLayout() {
  const panel = useSideNavPanel(options);
  const { subjects, current, past } = useSubjects();
  const snapshot = useRevisionStore();
  const [params] = useSearchParams();
  const { pathname } = useLocation();
  const subject = pickSubject(params.get("subject"), subjects, current, past);
  // "general" stands in only while no subject exists to scope to.
  const scope: number | "general" = subject?.id ?? "general";
  useEffect(() => {
    if (subject) rememberRevisionSubject(subject.id);
  }, [subject?.id]);
  const selectedTopic = params.get("topic") ?? "all";
  const availableTopics = topicsForSubject(snapshot.topics, scope);
  const topicScope: RevisionTopicScope =
    selectedTopic === "unsorted"
      ? "unsorted"
      : /^\d+$/.test(selectedTopic) &&
          availableTopics.some((topic) => topic.id === Number(selectedTopic))
        ? Number(selectedTopic)
        : "all";
  const now = useRevisionClock();
  const scoped = useMemo(() => {
    const keep = (row: { subject_id: number | null; topic_ids: number[] }) =>
      row.subject_id === (scope === "general" ? null : scope) &&
      matchesTopic(row, snapshot.topics, topicScope);
    return {
      ...snapshot,
      scope,
      topicScope,
      creationTopic: inheritedTopic(snapshot.topics, scope, topicScope),
      panel,
      notes: snapshot.notes.filter(keep),
      items: snapshot.items.filter(keep),
      sets: snapshot.sets.filter(keep),
      artifacts: snapshot.artifacts.filter(keep),
    };
  }, [snapshot, scope, topicScope, panel]);
  const topicCounts = useMemo(() => {
    const inScope = (row: { subject_id: number | null }) =>
      row.subject_id === (scope === "general" ? null : scope);
    const unsortedIds = new Set(
      snapshot.topics.filter((t) => t.is_unsorted).map((t) => t.id),
    );
    const counts = new Map<number, number>();
    let unsorted = 0;
    for (const rows of [snapshot.notes, snapshot.items, snapshot.sets, snapshot.artifacts])
      for (const row of rows) {
        if (!inScope(row)) continue;
        if (row.topic_ids.some((id) => unsortedIds.has(id))) unsorted++;
        for (const id of row.topic_ids) counts.set(id, (counts.get(id) ?? 0) + 1);
      }
    return { counts, unsorted };
  }, [snapshot, scope]);
  const scopeHref = (key: string) => `${pathname}?subject=${key}&topic=all`;
  const due = scoped.items.filter(
    (item) => item.status === "active" && new Date(item.due).getTime() <= now,
  ).length;
  useEffect(() => {
    void snapshot.reload();
    const refresh = () => void useRevisionStore.getState().reload();
    window.addEventListener(REVISION_UPDATED_EVENT, refresh);
    return () => window.removeEventListener(REVISION_UPDATED_EVENT, refresh);
  }, [snapshot.reload]);
  const href = (suffix: string) =>
    `/revision${suffix}?subject=${scope}&topic=${topicScope}`;

  return (
    <SideNavShell
      panel={panel}
      nav={
        <>
          <SideNav
            aria-label="Revision"
            width={panel.width}
            collapsed={panel.collapsed}
            animate={!panel.dragging}
            header={
              <>
                <SideNavHeading icon={Lightbulb}>Revision</SideNavHeading>
                {subject && (
                  <div className="px-2 pb-2">
                    <SubjectSwitcherMenu
                      field
                      label={displayCode(subject.code)}
                      icon={<SubjectIcon code={subject.code} size={14} />}
                      selected={String(subject.id)}
                      current={current}
                      past={past}
                      hrefFor={(s) => scopeHref(String(s.id))}
                    />
                  </div>
                )}
              </>
            }
          >
            <SideNavLink to={href("")} end icon={Note} label="Cheat sheet" />
            <SideNavLink
              to={href("/review")}
              icon={Cards}
              label="Review"
              trailing={<NewCountBadge count={due} />}
            />
            <SideNavLink to={href("/quizzes")} icon={Exam} label="Quizzes" />
            <SideNavLink
              to={href("/artifacts")}
              icon={SquaresFour}
              label="Artifacts"
            />
            {subject && (
              <TopicsGroup
                topics={availableTopics}
                counts={topicCounts.counts}
                unsorted={topicCounts.unsorted}
                selected={topicScope}
                subjectId={subject.id}
              />
            )}
          </SideNav>
          {!panel.collapsed && (
            <ResizeHandle
              onMouseDown={panel.onMouseDown}
              dragging={panel.dragging}
              label="Resize Revision sidebar"
              className="-mx-0.5"
            />
          )}
        </>
      }
    >
      <div className="flex h-full min-w-0 flex-1 flex-col">
        <Outlet context={scoped satisfies RevisionView} />
      </div>
    </SideNavShell>
  );
}

export const useRevisionData = () => useOutletContext<RevisionView>();
export const useRevisionPanel = () => useRevisionData().panel;
