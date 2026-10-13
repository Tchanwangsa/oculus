import { CheckCircle } from "@phosphor-icons/react";
import { useNavigate } from "react-router-dom";
import { FileRecency } from "@/components/files/FileRecency";
import { filePageHref, filePagePath, recordFileAccess } from "@/lib/files/openFile";
import { fmtDue } from "@/lib/format/format";
import { lessonIcon, lessonProgress, type EdLesson } from "@/lib/ed/lessons";
import { useLiveLessonIndex } from "@/lib/ed/slideStatus";

/** A lesson: its type, title, deadline and how many slides are completed.
 *  It opens full page in this pane, not beside the list: the lesson view
 *  brings its own slide sidebar. */
export function LessonRow({ lesson }: { lesson: EdLesson }) {
  const navigate = useNavigate();
  const index = useLiveLessonIndex(lesson.index);
  const Icon = lessonIcon(index?.lessonType ?? null);
  const progress = index && index.slides.length > 0 ? lessonProgress(index) : null;
  return (
    <button
      type="button"
      data-tab-href={filePageHref(lesson.file) ?? undefined}
      onClick={() => {
        recordFileAccess(lesson.file);
        navigate(filePagePath(lesson.file.subject_id, lesson.file.relative_path));
      }}
      className="w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-surface transition-colors"
    >
      <Icon size={14} className="shrink-0 text-reference-lesson" aria-hidden />
      <span title={lesson.title} className="min-w-0 flex-1 truncate text-[12px] font-medium leading-tight text-foreground">
        {lesson.title}
      </span>
      {index?.dueAt && (
        <span className="shrink-0 text-[11px] text-muted-foreground">Due {fmtDue(index.dueAt)}</span>
      )}
      <span className="shrink-0 w-10 flex items-center justify-end">
        {progress && (progress.done === progress.total ? (
          <CheckCircle size={13} weight="fill" className="text-success" aria-label="All slides completed" />
        ) : (
          <span
            title={`${progress.done} of ${progress.total} slides completed`}
            className="text-[11px] tabular-nums text-muted-foreground"
          >
            {progress.done}/{progress.total}
          </span>
        ))}
      </span>
      <span className="shrink-0 w-13 flex items-center justify-end">
        <FileRecency file={lesson.file} />
      </span>
    </button>
  );
}
