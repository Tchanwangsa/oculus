import { useMemo } from "react";
import { ChalkboardTeacher } from "@phosphor-icons/react";
import { SubjectLoading, SubjectPage, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useSubject } from "@/layouts/SubjectLayout";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { humanizeSlug } from "@/lib/format/format";
import { ListCard } from "@/components/ui/layout/PageParts";

/** A lesson's own markdown, `courses/<CODE>/lessons/<id>-<slug>.md`; its PDFs
 *  and challenge files sit one folder down and are reached from it. */
const LESSON_DOC = /^courses\/[^/]+\/lessons\/[^/]+\.md$/;

const lessonId = (filename: string) => Number(/^(\d+)-/.exec(filename)?.[1] ?? 0);

/**
 * The subject's Ed Lessons, as scraped to `lessons/<id>-<slug>.md` — one file
 * per lesson, every slide in order. Rows open in the side panel.
 */
export default function SubjectLessonsPage() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);

  const lessons = useMemo(
    () =>
      // Ed's lesson ids rise as lessons are made, so descending is newest
      // first. Not zero-padded, so compared as numbers.
      byCategory.lessons
        .filter((f) => LESSON_DOC.test(f.relative_path))
        .sort((a, b) => lessonId(b.filename) - lessonId(a.filename) || a.filename.localeCompare(b.filename)),
    [byCategory.lessons],
  );

  if (loading && lessons.length === 0) {
    return <SubjectLoading count={5} />;
  }

  if (lessons.length === 0) {
    return (
      <SubjectEmpty icon={<ChalkboardTeacher size={24} className="text-muted-foreground/40" />} title="No Ed lessons scraped.">
        <p className="text-xs text-muted-foreground/70 max-w-sm text-center">
          Run a sync — lessons come through the same Ed connection as Discussion,
          when the subject's Ed course has any.
        </p>
      </SubjectEmpty>
    );
  }

  return (
    <SubjectPage>
      <ListCard>
        {lessons.map((f) => (
          <button
            key={f.id}
            data-tab-href={filePageHref(f) ?? undefined}
            onClick={() => openFileSmart(f)}
            className="w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-surface transition-colors"
          >
            <span className="text-[12px] text-foreground truncate flex-1">
              {humanizeSlug(f.filename)}
            </span>
            <span className="shrink-0 w-13 flex items-center justify-end">
              <FileRecency file={f} />
            </span>
          </button>
        ))}
      </ListCard>
    </SubjectPage>
  );
}
