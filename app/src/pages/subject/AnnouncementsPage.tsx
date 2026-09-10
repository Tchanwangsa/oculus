import { useMemo } from "react";
import { Megaphone } from "@phosphor-icons/react";
import { Skeleton } from "@/components/ui/skeleton";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useSubject } from "@/layouts/SubjectLayout";
import { openFileSmart } from "@/lib/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { dateFromSlug, humanizeSlug } from "@/lib/format";

function fmtDay(d: Date): string {
  return d.toLocaleDateString("en-AU", { day: "numeric", month: "short" });
}

/** Every scraped announcement, newest first. Rows open in the peek. */
export default function SubjectAnnouncementsPage() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);

  const announcements = useMemo(
    () =>
      // Filenames are date-prefixed (YYYY-MM-DD-slug.md), so this is by date.
      [...byCategory.announcement].sort((a, b) =>
        b.filename.localeCompare(a.filename),
      ),
    [byCategory.announcement],
  );

  if (loading && announcements.length === 0) {
    return (
      <div className="page-scroll">
        <div className="mx-auto max-w-5xl px-6 py-6 space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} className="h-8 w-full" />
          ))}
        </div>
      </div>
    );
  }

  if (announcements.length === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2">
        <Megaphone size={24} className="text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">No announcements yet.</p>
      </div>
    );
  }

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-5xl px-6 py-5">
        <div className="rounded-lg border border-border divide-y divide-border-subtle overflow-hidden">
          {announcements.map((f) => {
            const posted = dateFromSlug(f.filename);
            return (
              <button
                key={f.id}
                onClick={() => openFileSmart(f)}
                className="w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-surface transition-colors"
              >
                <span className="text-[12px] text-foreground truncate flex-1">
                  {humanizeSlug(f.filename)}
                </span>
                {posted && (
                  <span className="shrink-0 text-[11px] text-muted-foreground">
                    {fmtDay(posted)}
                  </span>
                )}
                <FileRecency file={f} />
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
