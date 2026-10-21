import { useEffect, useMemo } from "react";
import { CaretDown, CaretRight, ChalkboardTeacher } from "@phosphor-icons/react";
import { SubjectLoading, SubjectPage, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useCourseFileData } from "@/hooks/data/useCourseFileData";
import { useCollapsedGroups } from "@/hooks/shell/useCollapsedGroups";
import { useSubject } from "@/layouts/SubjectLayout";
import { groupLessons, isLessonDoc, parseEdLesson } from "@/lib/ed/lessons";
import { refreshSlideStatuses } from "@/lib/ed/views";
import { LessonRow } from "./lessons/LessonRow";

const COLLAPSED_KEY = "oculus-lessons-modules-collapsed";

/**
 * The subject's Ed Lessons: each lesson's index, `lessons/<id>-<slug>.md`,
 * under its Ed module in Ed's order. Rows open the lesson view in the side
 * panel. Lesson files without front matter sit in a last "Other" group.
 */
export default function SubjectLessonsPage() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);
  const files = useMemo(() => byCategory.lessons.filter(isLessonDoc), [byCategory.lessons]);
  const lessons = useCourseFileData(files, loading, parseEdLesson);
  const groups = useMemo(() => {
    if (!lessons) return null;
    // A file the last read predates shows by its filename until it is read.
    const byId = new Map(lessons.map((lesson) => [lesson.file.id, lesson]));
    return groupLessons(files.map((file) => byId.get(file.id) ?? parseEdLesson("", file)));
  }, [files, lessons]);
  const { folded, setOpen } = useCollapsedGroups(COLLAPSED_KEY);
  const lessonIds = useMemo(
    () => (lessons ?? []).flatMap((lesson) => (lesson.index && lesson.lessonId > 0 ? [lesson.lessonId] : [])),
    [lessons],
  );
  const idsKey = lessonIds.join(",");
  // Ed's statuses, not the last sync's: the rows' counts read them.
  useEffect(() => refreshSlideStatuses(lessonIds), [idsKey]); // eslint-disable-line react-hooks/exhaustive-deps

  if ((loading && files.length === 0) || (files.length > 0 && groups == null)) {
    return <SubjectLoading count={5} />;
  }

  if (files.length === 0 || !groups) {
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
      {groups.map((group) => {
        const key = `${subject.id}:${group.key}`;
        const open = !folded.has(key);
        const count = group.lessons.length;
        return (
          <section key={group.key} className="mb-5 last:mb-0">
            <button
              type="button"
              aria-expanded={open}
              onClick={() => setOpen(key, !open)}
              className="mb-1 flex w-full items-center gap-2 py-2 text-left"
            >
              {open ? <CaretDown size={12} className="text-muted-foreground" /> : <CaretRight size={12} className="text-muted-foreground" />}
              <span className="truncate text-[13px] font-medium text-foreground">{group.label}</span>
              <span className="shrink-0 text-[11px] text-muted-foreground">
                {count} {count === 1 ? "lesson" : "lessons"}
              </span>
            </button>
            {open && (
              <div className="divide-y divide-border-subtle border-y border-border-subtle">
                {group.lessons.map((lesson) => <LessonRow key={lesson.file.id} lesson={lesson} />)}
              </div>
            )}
          </section>
        );
      })}
    </SubjectPage>
  );
}
