import { useEffect, useState } from "react";
import { SubjectLoading, SubjectPage } from "@/components/subjects/SubjectPage";
import { useSubjectOverview } from "@/hooks/data/overview";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useNow } from "@/hooks/ui/useNow";
import { useSubject } from "@/layouts/SubjectLayout";
import { lecturePagePath } from "@/lib/lectures/pagePath";
import { openBeside } from "@/lib/shell/tabRouters";
import { ActivityList } from "./overview/ActivityList";
import { AskDock } from "./overview/AskDock";
import { BehindWarning } from "./overview/BehindWarning";
import { ComingUpList } from "./overview/ComingUpList";
import { EmptyState } from "./overview/EmptyState";
import { weekName, weekRange } from "./overview/format";
import { OverviewHeader } from "./overview/Header";
import { useOverviewOpener } from "./overview/open";
import { SectionHeading } from "./overview/rows";
import { SideColumn } from "./overview/SideColumn";
import { WeekStrip } from "./overview/WeekStrip";

/**
 * A subject's "this week": the behind warning, the term's week strip, the
 * selected week's activity, the next deadlines, and beside them (below them in
 * a narrow pane) its events and the student's own work. The ask bar docks at
 * the bottom in the same column. Data is `useSubjectOverview`.
 */
export default function SubjectOverviewPage() {
  const subject = useSubject();
  // Not persisted: a fresh Overview always opens on the current week.
  const [picked, setPicked] = useState<number | null>(null);
  const data = useSubjectOverview(subject.id, picked);
  const { files, loaded: filesLoaded, error: filesError } = useSubjectFiles(subject.id);
  const opener = useOverviewOpener(files);
  const now = useNow();

  // Only the first load shows skeletons; a reload keeps the rows on screen.
  const [ready, setReady] = useState(false);
  if (!ready && !data.loading && (filesLoaded || filesError != null)) setReady(true);

  // Stamped on leaving, so this visit's dots stay up while the page is open.
  const { markVisited } = data;
  useEffect(() => markVisited, [markVisited]);

  const { weeks, current } = data.weeks;
  const pickedIndex = picked == null ? -1 : weeks.findIndex((week) => week.key === picked);
  // A reload that drops the picked week falls back to the current one.
  if (picked != null && pickedIndex < 0 && !data.loading) setPicked(null);

  if (!ready) return <SubjectLoading count={4} rowClassName="h-10" />;

  const selected = pickedIndex >= 0 ? pickedIndex : current;
  const week = weeks[selected];
  const thisWeek = selected === current;
  const nothingSynced = files.length === 0 && data.progress.every((p) => p.total === 0);

  return (
    // Chat's column is 760px of content: the bar warps into a composer its own width.
    <SubjectPage className="flex min-h-full max-w-[808px] flex-col pt-6 pb-0">
      <div className="@container min-w-0 space-y-5">
        <OverviewHeader subject={subject} revisionDue={data.revisionDue} />
        {data.error && (
          <p role="alert" className="px-0.5 text-[12px] text-destructive">
            Part of this page didn't load: {data.error}
          </p>
        )}
        {nothingSynced ? <EmptyState /> : (
          <>
            <BehindWarning
              behind={data.behind}
              onCatchUp={(lecture) => openBeside(lecturePagePath(lecture))}
              catchUpHref={data.behind.lectures[0] ? lecturePagePath(data.behind.lectures[0]) : undefined}
            />
            {week && (
              <WeekStrip strip={data.weeks} progress={data.progress} selected={selected} onSelect={setPicked} />
            )}
            <div className="grid gap-x-8 gap-y-7 @2xl:grid-cols-[minmax(0,1fr)_13rem]">
              <div className="min-w-0 space-y-7">
                <section>
                  <SectionHeading aside={week ? weekRange(week) : undefined}>
                    {thisWeek || !week ? "This week" : weekName(week)}
                  </SectionHeading>
                  <ActivityList
                    items={data.activity}
                    opener={opener}
                    empty={!week ? "No weeks yet: the timetable hasn't synced."
                      : thisWeek ? "Nothing new this week yet." : "Nothing new that week."}
                  />
                </section>
                <section>
                  <SectionHeading>Coming up</SectionHeading>
                  <ComingUpList items={data.comingUp} opener={opener} now={now} />
                </section>
              </div>
              <SideColumn events={data.events} yours={data.yours} opener={opener} now={now} thisWeek={thisWeek} />
            </div>
          </>
        )}
      </div>
      <AskDock subject={subject} />
    </SubjectPage>
  );
}
