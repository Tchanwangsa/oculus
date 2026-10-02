import { useMemo } from "react";
import { Megaphone } from "@phosphor-icons/react";
import { SubjectLoading, SubjectPage, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useSubject } from "@/layouts/SubjectLayout";
import { filePageHref, openFileSmart } from "@/lib/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { dateFromSlug, fmtShortDate, humanizeSlug } from "@/lib/format";
import { ListCard } from "@/components/ui/PageParts";

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
    return <SubjectLoading count={5} />;
  }

  if (announcements.length === 0) {
    return (
      <SubjectEmpty icon={<Megaphone size={24} className="text-muted-foreground/40" />} title="No announcements yet." />
    );
  }

  return (
    <SubjectPage>
      <ListCard>
        {announcements.map((f) => {
          const posted = dateFromSlug(f.filename);
          return (
            <button
              key={f.id}
              data-tab-href={filePageHref(f) ?? undefined}
              onClick={() => openFileSmart(f)}
              className="w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-surface transition-colors"
            >
              <span className="text-[12px] text-foreground truncate flex-1">
                {humanizeSlug(f.filename)}
              </span>
              {posted && (
                <span className="shrink-0 text-[11px] text-muted-foreground">
                  {fmtShortDate(posted)}
                </span>
              )}
              <FileRecency file={f} />
            </button>
          );
        })}
      </ListCard>
    </SubjectPage>
  );
}
