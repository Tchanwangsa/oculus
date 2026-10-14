import { getDb, searchLibraryFiles } from "@/lib/db";
import { notifyProjectsUpdated } from "./events";
import { touchTask } from "./touch";

/** A library file linked to a task, with what a row needs to draw and open it
 *  (`fileTitle`, `openFile`). */
export interface DbTaskFile {
  file_id: number;
  subject_id: number | null;
  filename: string;
  relative_path: string;
  file_type: string | null;
  category: string | null;
  subject_code: string | null;
}

/** The task's files, in the order they were attached. */
export async function getTaskFiles(taskId: number): Promise<DbTaskFile[]> {
  const db = await getDb();
  return db.select<DbTaskFile[]>(
    `SELECT f.id AS file_id, f.subject_id, f.filename, f.relative_path, f.file_type,
            f.category, s.code AS subject_code
       FROM task_files tf
       JOIN files f ON f.id = tf.file_id
       LEFT JOIN subjects s ON s.id = f.subject_id
      WHERE tf.task_id = $1
      ORDER BY tf.created_at ASC, f.filename ASC`,
    [taskId],
  );
}

/** Link a file to a task; linking it twice is a no-op. */
export async function addTaskFile(taskId: number, fileId: number): Promise<void> {
  const db = await getDb();
  await db.execute(
    `INSERT OR IGNORE INTO task_files (task_id, file_id) VALUES ($1, $2)`,
    [taskId, fileId],
  );
  await touchTask(taskId);
  notifyProjectsUpdated();
}

export async function removeTaskFile(taskId: number, fileId: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM task_files WHERE task_id = $1 AND file_id = $2`, [
    taskId,
    fileId,
  ]);
  await touchTask(taskId);
  notifyProjectsUpdated();
}

/** Files of the whole library matching a name, as ⌘K ranks them — the picker
 *  for attaching one to a task. An empty query lists recently opened files. */
export async function searchAttachableFiles(query: string, limit = 12): Promise<DbTaskFile[]> {
  const hits = await searchLibraryFiles(query, limit);
  return hits.map((f) => ({
    file_id: f.id,
    subject_id: f.subject_id,
    filename: f.filename,
    relative_path: f.relative_path,
    file_type: f.file_type,
    category: f.category,
    subject_code: f.subject_code,
  }));
}
