import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  ArrowRight,
  CaretLeft,
  CaretRight,
  FileText,
  VideoCamera,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useSubject } from "@/layouts/SubjectLayout";
import { getRecents, relativeTime, type RecentEntry } from "@/lib/recents";
import { dateFromSlug, humanizeSlug } from "@/lib/format";
import { filePageHref, openFileSmart } from "@/lib/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { lecturePagePath } from "@/lib/lectures";
import { getLectures, type DbFile, type Lecture } from "@/lib/db";

function fmtDay(d: Date): string {
  return d.toLocaleDateString("en-AU", { day: "numeric", month: "short" });
}

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
    return (
      <div className="page-scroll">
        <div className="mx-auto max-w-5xl px-6 py-6 space-y-3">
          <Skeleton className="h-24 w-full" />
          <Skeleton className="h-24 w-full" />
        </div>
      </div>
    );
  }

  const nothingSynced = files.length === 0 && (lectures?.length ?? 0) === 0;

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-5xl px-6 py-6 space-y-8">
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
                  {fmtDay(new Date(nextLecture.date))}
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
                    meta={posted ? fmtDay(posted) : undefined}
                  />
                );
              })}
            </div>
            {byCategory.announcement.length > recentAnnouncements.length && (
              <MoreLink to="announcements" label="All announcements" />
            )}
          </Section>
        )}
      </div>
    </div>
  );
}

// ── Pieces ────────────────────────────────────────────────────────────────────

function EmptyState() {
  const navigate = useNavigate();
  return (
    <div className="rounded-lg border border-border px-4 py-6 text-center">
      <p className="text-sm text-foreground">Nothing synced for this subject yet.</p>
      <p className="text-xs text-muted-foreground mt-1">
        Run a sync to pull its modules, pages, files and lectures.
      </p>
      <Button
        size="sm"
        variant="outline"
        className="mt-3 text-xs"
        onClick={() => navigate("/sync")}
      >
        Go to Sync
      </Button>
    </div>
  );
}

/**
 * Horizontal card row driven by the flanking arrow buttons (no visible
 * scrollbar). Arrows only render on their side while there is somewhere left
 * to go.
 */
function RecentsCarousel({
  recents, files, subjectId,
}: {
  recents: RecentEntry[];
  files: DbFile[];
  subjectId: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);

  const update = () => {
    const el = ref.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 1);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 1);
  };

  useLayoutEffect(() => {
    update();
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [recents]);

  const scrollBy = (dir: 1 | -1) => {
    const el = ref.current;
    if (!el) return;
    el.scrollBy({ left: dir * el.clientWidth * 0.8, behavior: "smooth" });
  };

  return (
    <div className="relative">
      <div
        ref={ref}
        onScroll={update}
        className="flex gap-2.5 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {recents.map((entry) => (
          <RecentCard
            key={`${entry.kind}:${entry.ref}`}
            entry={entry}
            files={files}
            subjectId={subjectId}
          />
        ))}
      </div>

      {canLeft && <CarouselArrow side="left" onClick={() => scrollBy(-1)} />}
      {canRight && <CarouselArrow side="right" onClick={() => scrollBy(1)} />}
    </div>
  );
}

function CarouselArrow({
  side, onClick,
}: {
  side: "left" | "right";
  onClick: () => void;
}) {
  const Icon = side === "left" ? CaretLeft : CaretRight;
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={side === "left" ? "Scroll back" : "Scroll forward"}
      className={cn(
        "absolute top-1/2 -translate-y-1/2 z-10 flex h-7 w-7 items-center justify-center rounded-full",
        "border border-border bg-background shadow-sm text-muted-foreground",
        "hover:text-foreground hover:bg-surface transition-colors",
        side === "left" ? "-left-3.5" : "-right-3.5",
      )}
    >
      <Icon size={12} />
    </button>
  );
}

function RecentCard({
  entry, files, subjectId,
}: {
  entry: RecentEntry;
  files: DbFile[];
  subjectId: number;
}) {
  const navigate = useNavigate();
  const Icon = entry.kind === "lecture" ? VideoCamera : FileText;

  const open = () => {
    if (entry.kind === "lecture") {
      navigate(
        `/subjects/${subjectId}/lecture?id=${encodeURIComponent(entry.ref)}&t=${encodeURIComponent(entry.title)}`,
      );
      return;
    }
    const file = files.find((f) => f.relative_path === entry.ref);
    if (file) openFileSmart(file);
  };

  return (
    <button
      onClick={open}
      className="shrink-0 w-44 rounded-lg border border-border px-3 py-2.5 text-left hover:bg-surface transition-colors"
    >
      <Icon size={15} className="text-muted-foreground" />
      <p className="mt-2 text-[12px] font-medium text-foreground line-clamp-2 break-words leading-snug h-[2.75em]">
        {entry.title}
      </p>
      <p className="mt-1.5 text-[11px] text-muted-foreground">
        {relativeTime(entry.visitedAt)}
      </p>
    </button>
  );
}

function Section({
  title, children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h2 className="mb-2.5 text-[13px] font-semibold text-foreground">{title}</h2>
      {children}
    </section>
  );
}

/** Opens the file in the side panel. */
function FileLink({
  file, label, meta,
}: {
  file: DbFile;
  label: string;
  meta?: string;
}) {
  return (
    <button
      data-tab-href={filePageHref(file) ?? undefined}
      onClick={() => openFileSmart(file)}
      className="w-full flex items-center gap-3 rounded-md px-2 py-1.5 -mx-2 text-left text-muted-foreground hover:bg-surface hover:text-foreground transition-colors"
    >
      <span className="text-[12px] truncate flex-1">{label}</span>
      {meta && <span className="text-[11px] opacity-60 shrink-0">{meta}</span>}
      <FileRecency file={file} />
    </button>
  );
}

function MoreLink({ to, label }: { to: string; label: string }) {
  return (
    <Link
      to={to}
      className="mt-2 inline-flex items-center gap-1 text-[11px] text-brand hover:underline"
    >
      {label}
      <ArrowRight size={10} />
    </Link>
  );
}
