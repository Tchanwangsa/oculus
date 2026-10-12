import { getFileByRelativePath, getLecture } from "@/lib/db";
import { fileTitle } from "@/lib/files/openFile";
import { parsedSourceOf } from "@/lib/citations";
import { fmtBytes } from "@/lib/format/format";
import { fmtDurationSecs } from "@/lib/lectures/format";
import type { ReferenceDetails } from "./ReferencePreview";

export async function fileReferenceDetails(path: string): Promise<ReferenceDetails | null> {
  const lectureId = /^lectures\/([^/]+)\//.exec(path)?.[1];
  if (lectureId) {
    const lecture = await getLecture(lectureId);
    return lecture ? { title: lecture.title, meta: fmtDurationSecs(lecture.duration_seconds) } : null;
  }
  const file = await getFileByRelativePath(parsedSourceOf(path) ?? path);
  return file ? { title: fileTitle(file), meta: file.size_bytes == null ? undefined : fmtBytes(file.size_bytes) } : null;
}

/** Summaries stay plain text: a hover must never mount nested embeds or controls. */
function summary(text: string | null): string | undefined {
  return text?.replace(/```[\s\S]*?```/g, "").replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1").replace(/[#*_`>]/g, "").replace(/\s+/g, " ").trim().slice(0, 240) || undefined;
}

export async function routeReferenceDetails(kind: string, id: number | string): Promise<ReferenceDetails | null> {
  if (kind === "Lecture") {
    const lecture = await getLecture(String(id));
    return lecture ? { title: lecture.title, meta: fmtDurationSecs(lecture.duration_seconds) } : null;
  }
  if (kind === "Project") {
    const { getProject } = await import("@/lib/planning/projects");
    const project = await getProject(Number(id));
    return project ? { title: project.name, description: summary(project.brief), meta: project.subject_code ?? undefined } : null;
  }
  const { getDb } = await import("@/lib/db");
  const db = await getDb();
  const rows = await db.select<{ title: string; body: string | null; done_at: string | null }[]>(
    "SELECT title, body, done_at FROM project_tasks WHERE id = $1", [id],
  );
  const task = rows[0];
  return task ? { title: task.title, description: summary(task.body), meta: task.done_at ? "Completed" : undefined } : null;
}
