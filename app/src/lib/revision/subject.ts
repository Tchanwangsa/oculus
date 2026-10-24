import { getSubjects } from "@/lib/db";
import { getDb } from "@/lib/db/connection";
import { lastRevisionSubject } from "./lastSubject";

/**
 * The subject something saved from a chat files under: the thread's, else
 * the one Revision last showed, else the first current subject. Null only
 * when nothing is synced, and the writer then refuses the save.
 */
export async function widgetSubject(
  thread: number | null,
): Promise<number | null> {
  if (thread != null) {
    const db = await getDb();
    const [row] = await db.select<{ subject_id: number | null }[]>(
      "SELECT subject_id FROM harness_threads WHERE id=$1",
      [thread],
    );
    if (row?.subject_id != null) return row.subject_id;
  }
  const subjects = await getSubjects();
  const last = lastRevisionSubject();
  return (
    subjects.find((s) => s.id === last)?.id ??
    subjects.find((s) => s.is_current)?.id ??
    subjects[0]?.id ??
    null
  );
}
