import { getDb } from "./connection";

/** A calendar row the user wrote. Kept out of `calendar_events`, which every
 *  sync replaces. `subject_code` is NULL for a personal event. */
export interface DbLocalEvent {
  id: number;
  subject_id: number | null;
  subject_code: string | null;
  kind: string;              // 'due' | 'class' | 'note'
  title: string;
  start_at: string;          // ISO8601
  end_at: string | null;
  all_day: number;
  notes: string | null;
  source: string;            // 'manual' (old rows may say 'automation')
  created_at: string;
}

/** Every local event, oldest first (unwindowed, like `getCalendarEvents`). */
export async function getLocalEvents(): Promise<DbLocalEvent[]> {
  const db = await getDb();
  return db.select<DbLocalEvent[]>(
    `SELECT le.id, le.subject_id, s.code AS subject_code, le.kind, le.title,
            le.start_at, le.end_at, le.all_day, le.notes, le.source, le.created_at
       FROM local_events le
       LEFT JOIN subjects s ON s.id = le.subject_id
      ORDER BY le.start_at ASC`,
  );
}

/** A local event as the editor holds it. `subjectId` null = "Personal";
 *  dates are ISO 8601 instants. */
export interface LocalEventInput {
  subjectId: number | null;
  /** `note`, `class` or `due`. */
  kind: string;
  title: string;
  startAt: string;
  endAt: string | null;
  allDay: boolean;
  notes: string | null;
}

/** Write a local event and return its id (from `execute()`, for the pooled-
 *  connection reason in `startSyncRun`). */
export async function createLocalEvent(input: LocalEventInput): Promise<number> {
  const db = await getDb();
  const res = await db.execute(
    `INSERT INTO local_events
       (subject_id, kind, title, start_at, end_at, all_day, notes, source)
     VALUES ($1, $2, $3, $4, $5, $6, $7, 'manual')`,
    [
      input.subjectId,
      input.kind,
      input.title,
      input.startAt,
      input.endAt,
      input.allDay ? 1 : 0,
      input.notes,
    ],
  );
  if (res.lastInsertId == null) throw new Error("local event insert returned no id");
  return res.lastInsertId;
}

/** Rewrite every editable column. `source` is left as it was. */
export async function updateLocalEvent(
  id: number,
  input: LocalEventInput,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE local_events
        SET subject_id = $1, kind = $2, title = $3, start_at = $4,
            end_at = $5, all_day = $6, notes = $7
      WHERE id = $8`,
    [
      input.subjectId,
      input.kind,
      input.title,
      input.startAt,
      input.endAt,
      input.allDay ? 1 : 0,
      input.notes,
      id,
    ],
  );
}

export async function deleteLocalEvent(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM local_events WHERE id = $1`, [id]);
}
