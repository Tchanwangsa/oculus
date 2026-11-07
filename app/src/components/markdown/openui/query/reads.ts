import { getDb, getSubjects, getAllLectures, getCalendarEvents, getLocalEvents } from "@/lib/db";
import { getAllTasks, getTasks, getProject, getProjects, getTaskCounts, getTaskTags } from "@/lib/planning/projects";
import { getNotes, getItems } from "@/lib/revision/reads";

/** Only local database reads. No sync, fetch, command name or SQL from the program. */
async function getFileCounts(ids: number[]) {
  if (!ids.length) return new Map<number, number>();
  const db = await getDb();
  const rows = await db.select<{ task_id: number; count: number }[]>(
    `SELECT task_id, COUNT(*) AS count FROM task_files WHERE task_id IN (${ids.map((_, i) => `$${i + 1}`).join(",")}) GROUP BY task_id`, ids);
  return new Map(rows.map(row => [row.task_id, row.count]));
}
export const localReads = { getFileCounts, getSubjects, getAllLectures, getCalendarEvents, getLocalEvents, getAllTasks, getTasks, getProject, getProjects, getTaskCounts, getTaskTags, getNotes, getItems };
export type Reads = typeof localReads;
