import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { ArrowRight } from "@phosphor-icons/react";
import { SubjectLoading, SubjectPage } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useSubject } from "@/layouts/SubjectLayout";
import { getRecents, type RecentEntry } from "@/lib/activity/recents";
import { dateFromSlug, fmtShortDate, humanizeSlug } from "@/lib/format/format";
import { lecturePagePath } from "@/lib/lectures";
import { getLectures, type Lecture } from "@/lib/db";
import { EmptyState } from "./overview/EmptyState";
import { FileLink, MoreLink, Section } from "./overview/parts";
import { RecentsCarousel } from "./overview/RecentsCarousel";

/**
 * Subject home: where you left off, then what's new. Counts and per-tab stats
 * deliberately don't live here — the tabs themselves are one click away.
 */
export default function SubjectOverviewPage() {
  const subject = useSubject();
  const { files, loading: filesLoading, byCategory } = useSubjectFiles(subject.id);
  const [lectures, setLectures] = useState<Lecture[] | null>(null);
  // Read synchronously so the row never flashes empty on first paint.
  const [recents] = useState<RecentEntry[]>(() => getRecents(subject.id));

  useEffect(() => {
    let cancelled = false;
    getLectures(subject.id)
      .then((rows) => !cancelled && setLectures(rows))
      .catch(() => !cancelled && setLectures([]));
    return () => {
      cancelled = true;
    };
  }, [subject.id]);

  const recentAnnouncements = useMemo(
    () =>
      [...byCategory.announcement]
        .sort((a, b) => b.filename.localeCompare(a.filename)) // date-prefixed
        .slice(0, 4),
    [byCategory.announcement],
  );

  const nextLecture = useMemo(() => {
    const rows = lectures ?? [];
    return rows.find((l) => !l.completed) ?? null;
  }, [lectures]);

  if (filesLoading && files.length === 0) {
    return <SubjectLoading count={2} rowClassName="h-24" spacing="space-y-3" />;
  }

  const nothingSynced = files.length === 0 && (lectures?.length ?? 0) === 0;

  return (
    <SubjectPage className="py-6 space-y-8">
      {nothingSynced && <EmptyState />}

      {recents.length > 0 && (
        <Section title="Recently visited">
          <RecentsCarousel recents={recents} files={files} subjectId={subject.id} />
        </Section>
      )}

      {nextLecture && (
        <Section title="Up next">
          <Link
            to={lecturePagePath(nextLecture)}
            className="flex items-center gap-3 rounded-lg border border-border px-3.5 py-3 hover:bg-surface transition-colors"
          >
            <div className="min-w-0 flex-1">
              <p className="text-[13px] font-medium text-foreground truncate">
                {nextLecture.title}
              </p>
              <p className="mt-0.5 text-[11px] text-muted-foreground">
                {fmtShortDate(new Date(nextLecture.date))}
                {nextLecture.progress_seconds > 5 ? " · in progress" : ""}
              </p>
            </div>
            <ArrowRight size={13} className="text-muted-foreground shrink-0" />
          </Link>
        </Section>
      )}

      {(byCategory.home.length > 0 || byCategory.syllabus.length > 0) && (
        <Section title="Start here">
          <div className="space-y-0.5">
            {byCategory.home.map((f) => (
              <FileLink key={f.id} file={f} label="Course overview" />
            ))}
            {byCategory.syllabus.map((f) => (
              <FileLink key={f.id} file={f} label="Syllabus" />
            ))}
          </div>
        </Section>
      )}

      {recentAnnouncements.length > 0 && (
        <Section title="Announcements">
          <div className="space-y-0.5">
            {recentAnnouncements.map((f) => {
              const posted = dateFromSlug(f.filename);
              return (
                <FileLink
                  key={f.id}
                  file={f}
                  label={humanizeSlug(f.filename)}
                  meta={posted ? fmtShortDate(posted) : undefined}
                />
              );
            })}
          </div>
          {byCategory.announcement.length > recentAnnouncements.length && (
            <MoreLink to="announcements" label="All announcements" />
          )}
        </Section>
      )}
    </SubjectPage>
  );
}
