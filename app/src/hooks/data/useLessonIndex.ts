import { useMemo } from "react";
import type { DbFile } from "@/lib/db";
import { isLessonDoc, parseEdLesson, type LessonIndex } from "@/lib/ed/lessons";
import { useCourseFileData } from "./useCourseFileData";

/** A lesson index's front matter, when `file` is a lesson's own markdown.
 *  `checked` is false while that file is still being read; for any other
 *  file it is true at once and `index` is null. */
export function useLessonIndex(file: DbFile | null): { index: LessonIndex | null; checked: boolean } {
  const lessonDoc = file != null && file.category === "lessons" && isLessonDoc(file);
  const list = useMemo(() => (lessonDoc && file ? [file] : []), [lessonDoc, file]);
  const data = useCourseFileData(list, false, parseEdLesson);
  if (!lessonDoc) return { index: null, checked: true };
  // The last answer may be another file's until this one is read.
  const entry = data?.[0];
  return entry && entry.file.id === file.id
    ? { index: entry.index, checked: true }
    : { index: null, checked: false };
}
