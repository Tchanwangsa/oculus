import { getDb } from "./connection";
import type { Subject } from "./types";

export interface CanvasCourseRaw {
  id: number;
  course_code: string;
  name: string;
  workflow_state: string;
  term?: { name: string };
  _oculus_term_sort?: string | null;
  _oculus_is_current: boolean;
}

export async function upsertSubjects(courses: CanvasCourseRaw[]): Promise<void> {
  const db = await getDb();
  for (const c of courses) {
    await db.execute(
      `INSERT INTO subjects (id, code, name, term_name, term_sort, is_current, workflow_state, selected)
       VALUES ($1, $2, $3, $4, $8, $5, $6, $7)
       ON CONFLICT(id) DO UPDATE SET
         name           = excluded.name,
         term_name      = excluded.term_name,
         term_sort      = excluded.term_sort,
         is_current     = excluded.is_current,
         workflow_state = excluded.workflow_state`,
      [
        c.id,
        c.course_code,
        c.name,
        c.term?.name ?? null,
        c._oculus_is_current ? 1 : 0,
        c.workflow_state,
        // New subjects start selected only if current; ON CONFLICT keeps the user's choice.
        c._oculus_is_current ? 1 : 0,
        c._oculus_term_sort ?? null,
      ]
    );
  }
}

export async function getSubjects(): Promise<Subject[]> {
  const db = await getDb();
  // `last_synced_at` is derived: the latest completed run that targeted the subject.
  const rows = await db.select<Subject[]>(
    `WITH last_sync AS (
       SELECT j.value AS code, MAX(r.finished_at) AS finished_at
       FROM sync_runs r, json_each(r.subject_codes) j
       WHERE r.status = 'completed'
       GROUP BY j.value
     )
     SELECT s.*, ls.finished_at AS last_synced_at
     FROM subjects s LEFT JOIN last_sync ls ON ls.code = s.code
     ORDER BY s.term_sort IS NULL, s.term_sort DESC, s.name ASC`
  );

  // `is_current` is recomputed across every row rather than read from the
  // column, which each course fetch stamps only for the courses it returned.
  const latest = rows
    .filter((r) => r.workflow_state === "available" && r.term_sort)
    .reduce<string | null>(
      (best, r) => (best == null || (r.term_sort ?? "") > best ? r.term_sort : best),
      null,
    );

  return rows
    .map((r) => ({
      ...r,
      is_current: r.workflow_state === "available" && latest != null && r.term_sort === latest,
      selected: !!r.selected,
    }))
    // Current term first; sort is stable, so the query's order holds within groups.
    .sort((a, b) => Number(b.is_current) - Number(a.is_current));
}

export async function setSubjectSelected(id: number, selected: boolean): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE subjects SET selected = $1 WHERE id = $2`, [selected ? 1 : 0, id]);
}
