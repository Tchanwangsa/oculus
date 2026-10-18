import { CheckSquare, File, Kanban, Scribble, VideoCamera } from "@phosphor-icons/react";
import { categoryIconFor, fileIconFor, isLessonAsset, isSheetFile } from "./fileTypes";
import { lecturePageId } from "@/lib/lectures/pagePath";

/** Reference colours identify content, independently of the shell accent. */
export function fileReferenceAppearance(file: { category: string | null; filename: string }, path = "") {
  if (path.startsWith("lectures/")) return { Icon: VideoCamera, tone: "text-reference-lecture", kind: "Lecture" };
  const category = file.category;
  if (category === "canvas") return { Icon: Scribble, tone: "text-reference-page", kind: "Canvas" };
  if (category === "page" || category === "document" || category === "home" || category === "syllabus")
    return { Icon: File, tone: "text-reference-page", kind: category === "document" ? "Note" : "Page" };
  if (category === "ed") return { Icon: categoryIconFor(file), tone: "text-reference-discussion", kind: "Discussion" };
  if (category === "lessons" && !isLessonAsset(file))
    return { Icon: categoryIconFor(file), tone: "text-reference-lesson", kind: "Lesson" };
  if (/\.pdf$/i.test(file.filename)) return { Icon: fileIconFor(file.filename), tone: "text-reference-pdf", kind: "PDF" };
  if (isSheetFile(file.filename)) return { Icon: fileIconFor(file.filename), tone: "text-reference-lesson", kind: "Spreadsheet" };
  return { Icon: categoryIconFor(file), tone: "text-brand", kind: category === "assignment" ? "Assignment" : "File" };
}

/** Only recognised app routes become internal references; other relative links stay text. */
export function routeReferenceAppearance(href: string) {
  const pathname = href.split("?")[0];
  const task = /^\/(?:projects\/\d+\/)?tasks\/(\d+)\/?$/.exec(pathname);
  if (task) return { Icon: CheckSquare, tone: "text-reference-task", kind: "Task", id: Number(task[1]) };
  const project = /^\/projects\/(\d+)\/?$/.exec(pathname);
  if (project) return { Icon: Kanban, tone: "text-reference-project", kind: "Project", id: Number(project[1]) };
  const lecture = lecturePageId(href);
  if (lecture)
    return { Icon: VideoCamera, tone: "text-reference-lecture", kind: "Lecture", id: lecture };
  return null;
}
