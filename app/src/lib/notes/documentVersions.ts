import { getDb, type DbFile } from "@/lib/db";
import { createDocument, lastNoteWrite, quickHash, saveDocument } from "@/lib/notes/documents";
import { fmtShortDate, sqliteUtcToMs } from "@/lib/format/format";

/**
 * A note's saved versions, in `document_versions` (migration 39): the whole
 * text each time, keyed by the file's row id so a rename keeps its history.
 * **Checkpoints** are the student's own ("Save version", numbered v1, v2…)
 * and never pruned; **snapshots** are taken for them — when a note opens,
 * every {@link AUTO_SNAPSHOT_MS} of editing, when its last editor closes, and
 * before an in-place restore — skipped when the text matches the newest
 * version, and thinned by {@link versionsToPrune}.
 */

export type VersionKind = "checkpoint" | "auto" | "external" | "restore";

/** A version without its text; {@link versionText} reads that. */
export interface DocumentVersion {
  id: number;
  file_id: number;
  /** Checkpoints only: 1, 2, 3… per note. */
  number: number | null;
  label: string | null;
  kind: VersionKind;
  hash: string;
  /** SQLite `datetime('now')`: UTC, `YYYY-MM-DD HH:MM:SS`. */
  created_at: string;
  /** Characters of text, for the list. */
  length: number;
}

/** Why a session asks for a snapshot; {@link sessionSnapshot} picks the kind. */
export type SnapshotReason = "open" | "interval" | "close";

/** Fired with `{ fileId }` whenever a note's versions change. */
export const DOCUMENT_VERSIONS_EVENT = "oculus:document-versions-changed";

/** Editing time between automatic snapshots. */
export const AUTO_SNAPSHOT_MS = 10 * 60_000;

/** The label an `external` snapshot carries. */
export const CHANGED_OUTSIDE_LABEL = "Changed outside the editor";

const DAY_MS = 86_400_000;
/** Every snapshot younger than this stays. */
const KEEP_ALL_MS = DAY_MS;
/** Past this, snapshots go; between the two, the newest per UTC day stays. */
const KEEP_DAILY_MS = 30 * DAY_MS;

/** Everything but the text, which can be long and the list never shows. */
const COLUMNS = `id, file_id, number, label, kind, hash, created_at, length(text) AS length`;

const announce = (fileId: number) => {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent(DOCUMENT_VERSIONS_EVENT, { detail: { fileId } }));
};

const blankToNull = (label: string | null | undefined) => label?.trim() || null;

/** Hex sha-256 of the text: what dedupe compares. */
export async function textHash(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}

// A snapshot reads the newest hash and then writes, so two for one note at
// once (a close and a reopen) would both miss each other. One at a time per note.
const queues = new Map<number, Promise<unknown>>();

function serial<T>(fileId: number, run: () => Promise<T>): Promise<T> {
  const next = (queues.get(fileId) ?? Promise.resolve()).then(run, run);
  const tail = next.catch(() => {});
  queues.set(fileId, tail);
  void tail.then(() => {
    if (queues.get(fileId) === tail) queues.delete(fileId);
  });
  return next;
}

async function versionById(id: number): Promise<DocumentVersion> {
  const db = await getDb();
  const rows = await db.select<DocumentVersion[]>(
    `SELECT ${COLUMNS} FROM document_versions WHERE id = $1`,
    [id],
  );
  if (!rows[0]) throw new Error(`no version ${id}`);
  return rows[0];
}

async function newestHash(fileId: number): Promise<string | null> {
  const db = await getDb();
  const rows = await db.select<{ hash: string }[]>(
    `SELECT hash FROM document_versions WHERE file_id = $1
     ORDER BY created_at DESC, id DESC LIMIT 1`,
    [fileId],
  );
  return rows[0]?.hash ?? null;
}

/** Newest first, without text. */
export async function listVersions(fileId: number): Promise<DocumentVersion[]> {
  const db = await getDb();
  return db.select<DocumentVersion[]>(
    `SELECT ${COLUMNS} FROM document_versions WHERE file_id = $1
     ORDER BY created_at DESC, id DESC`,
    [fileId],
  );
}

export async function versionText(id: number): Promise<string> {
  const db = await getDb();
  const rows = await db.select<{ text: string }[]>(
    `SELECT text FROM document_versions WHERE id = $1`,
    [id],
  );
  if (!rows[0]) throw new Error(`no version ${id}`);
  return rows[0].text;
}

/** The student's "Save version": always written, numbered one past the
 *  note's highest checkpoint. */
export async function saveCheckpoint(
  fileId: number,
  text: string,
  label?: string,
): Promise<DocumentVersion> {
  const hash = await textHash(text);
  const db = await getDb();
  // One statement, so two quick saves cannot take the same number.
  const res = await db.execute(
    `INSERT INTO document_versions (file_id, number, label, kind, text, hash)
     SELECT $1, COALESCE(MAX(number), 0) + 1, $2, 'checkpoint', $3, $4
     FROM document_versions WHERE file_id = $1`,
    [fileId, blankToNull(label), text, hash],
  );
  if (res.lastInsertId == null) throw new Error("checkpoint insert returned no id");
  const version = await versionById(res.lastInsertId);
  announce(fileId);
  return version;
}

/** Write a snapshot, then drop the note's snapshots {@link versionsToPrune}
 *  names. Callers have already checked it differs from the newest. */
async function record(
  fileId: number,
  text: string,
  hash: string,
  kind: Exclude<VersionKind, "checkpoint">,
  label: string | null,
): Promise<DocumentVersion> {
  const db = await getDb();
  const res = await db.execute(
    `INSERT INTO document_versions (file_id, label, kind, text, hash)
     VALUES ($1, $2, $3, $4, $5)`,
    [fileId, label, kind, text, hash],
  );
  if (res.lastInsertId == null) throw new Error("snapshot insert returned no id");
  const version = await versionById(res.lastInsertId);
  const prune = versionsToPrune(await listVersions(fileId), new Date());
  if (prune.length) {
    await db.execute(
      `DELETE FROM document_versions WHERE id IN (${prune.map((_, i) => `$${i + 1}`).join(", ")})`,
      prune,
    );
  }
  announce(fileId);
  return version;
}

/** An automatic version; null (nothing written) when `text` matches the
 *  note's newest version. Prunes the note's snapshots after writing. */
export async function snapshot(
  fileId: number,
  text: string,
  kind: Exclude<VersionKind, "checkpoint">,
  label?: string,
): Promise<DocumentVersion | null> {
  return serial(fileId, async () => {
    const hash = await textHash(text);
    if (hash === (await newestHash(fileId))) return null;
    return record(fileId, text, hash, kind, blankToNull(label));
  });
}

/** Whether a note opened with text unlike its newest version was written by
 *  someone else: the app's own last write (`lastNoteWrite`) is known and is
 *  not this text. With no record it is taken as the app's own. */
export function changedOutside(written: string | null, text: string): boolean {
  return written !== null && written !== quickHash(text);
}

/** What `documentSessions` calls. `open` becomes `external` when the note
 *  already has versions, the text on disk is not the newest, and the app did
 *  not write it ({@link changedOutside}) — and `auto` otherwise. */
export async function sessionSnapshot(
  fileId: number,
  text: string,
  reason: SnapshotReason,
): Promise<void> {
  // Never rejects: it runs beside the save loop, and a lost snapshot must
  // not look like a lost save.
  try {
    await serial(fileId, async () => {
      const hash = await textHash(text);
      const newest = await newestHash(fileId);
      if (hash === newest) return;
      const external =
        reason === "open" && newest !== null && changedOutside(lastNoteWrite(fileId), text);
      await record(
        fileId,
        text,
        hash,
        external ? "external" : "auto",
        external ? CHANGED_OUTSIDE_LABEL : null,
      );
    });
  } catch (e) {
    console.error(`[oculus] ${reason} snapshot of note ${fileId} failed`, e);
  }
}

/** Rename a version; null or blank clears the label. */
export async function relabelVersion(id: number, label: string | null): Promise<void> {
  const { file_id } = await versionById(id);
  const db = await getDb();
  await db.execute(`UPDATE document_versions SET label = $1 WHERE id = $2`, [
    blankToNull(label),
    id,
  ]);
  announce(file_id);
}

export async function deleteVersion(id: number): Promise<void> {
  const { file_id } = await versionById(id);
  const db = await getDb();
  await db.execute(`DELETE FROM document_versions WHERE id = $1`, [id]);
  announce(file_id);
}

/** Snapshot ids to drop: every snapshot from the last 24 h stays, then the
 *  newest per UTC day up to 30 days, then none. Checkpoints always stay. */
export function versionsToPrune(versions: DocumentVersion[], now: Date): number[] {
  const prune: number[] = [];
  // The newest so far per UTC day, among snapshots past the 24 h window.
  const dayNewest = new Map<string, { id: number; ms: number }>();
  for (const v of versions) {
    if (v.kind === "checkpoint") continue;
    const ms = sqliteUtcToMs(v.created_at);
    if (ms == null) continue;
    const age = now.getTime() - ms;
    if (age < KEEP_ALL_MS) continue;
    if (age > KEEP_DAILY_MS) {
      prune.push(v.id);
      continue;
    }
    const day = new Date(ms).toISOString().slice(0, 10);
    const kept = dayNewest.get(day);
    if (!kept) {
      dayNewest.set(day, { id: v.id, ms });
    } else if (ms > kept.ms || (ms === kept.ms && v.id > kept.id)) {
      prune.push(kept.id);
      dayNewest.set(day, { id: v.id, ms });
    } else {
      prune.push(v.id);
    }
  }
  return prune;
}

/** A snapshot's local time: "5 Oct" and "2:02 pm", as `fmtClock` writes it. */
function stamp(version: DocumentVersion): { day: string; time: string } {
  const d = new Date(sqliteUtcToMs(version.created_at) ?? NaN);
  const time = d.toLocaleTimeString("en-AU", { hour: "numeric", minute: "2-digit" });
  return { day: fmtShortDate(d), time };
}

/** "v3 · Before the exam", "v3", or a snapshot's local time ("5 Oct, 2:02 pm"). */
export function versionTitle(version: DocumentVersion): string {
  if (version.kind === "checkpoint" && version.number != null) {
    return version.label ? `v${version.number} · ${version.label}` : `v${version.number}`;
  }
  const { day, time } = stamp(version);
  return `${day}, ${time}`;
}

/** What a copy's title ends with: "v3", or "5 Oct 2.02 pm" — no `:`, since the
 *  title is a filename. */
export function copySuffix(version: DocumentVersion): string {
  if (version.kind === "checkpoint" && version.number != null) return `v${version.number}`;
  const { day, time } = stamp(version);
  return `${day} ${time.replace(":", ".")}`;
}

/** A new note holding the version's text, titled "<title> (v3)" or
 *  "<title> (5 Oct 2.02 pm)"; a taken title steps aside as usual. */
export async function restoreAsCopy(
  subject: { id: number; code: string },
  file: DbFile,
  version: DocumentVersion,
): Promise<DbFile> {
  // Imported here: `openFile` pulls in the lecture player, which needs a
  // DOM, and `documentSessions` (with its tests) imports this module.
  const { fileTitle } = await import("@/lib/files/openFile");
  const text = await versionText(version.id);
  const row = await createDocument(subject, `${fileTitle(file)} (${copySuffix(version)})`);
  await saveDocument(row, text);
  // The copy's history starts at the text it was made from. The note exists
  // by now, so a failed baseline is logged rather than failing the restore.
  await snapshot(row.id, text, "auto").catch((e) =>
    console.error(`[oculus] baseline snapshot of note ${row.id} failed`, e),
  );
  return row;
}
