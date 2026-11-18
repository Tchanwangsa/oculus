/** Rows for the student's own files that arrived without one — written by a
 *  chat agent, Finder or another app straight into a subject's folder — so
 *  they are parsed and searchable like an upload. Rust renames a `.canvas`
 *  drawing to `.whiteboard` and inserts the rows (`reconcile_own_files`,
 *  `app/src-tauri/src/library/drive/ops/reconcile/`, never deletes); this kicks each new file's parse as `recordUploads` does,
 *  then runs the note and drawing reconcilers, which also drop rows whose
 *  file is gone. docs/frontend.md has when it runs. */
import { invoke } from "@tauri-apps/api/core";
import { getDb } from "@/lib/db/connection";
import { getSubjects } from "@/lib/db/subjects";
import { parseFile } from "@/lib/files/courseFiles";
import { isPipelineFile } from "@/lib/files/fileTypes";
import { UPLOADS_CHANGED_EVENT, type ImportedFile } from "@/lib/files/uploads";
import { reconcileDocuments } from "@/lib/notes/documents";
import { reconcileWhiteboards, reconcileNotebooks } from "@/lib/whiteboard/files";

export interface ReconcileSubject {
  id: number;
  code: string;
}

/** Calls for one subject coalesce while they keep coming this often. */
const DEBOUNCE_MS = 1500;
/** A run that is not forced is skipped this soon after the last one, so a
 *  listing that reloads on the run's own `drive-changed` and asks again
 *  settles instead of looping. */
const COOLDOWN_MS = 10_000;

/** One pass for `subject`: Rust's rows, their parses, then the notes' and
 *  drawings' own reconcilers, in that order so they see Rust's rows. */
export async function reconcileOwnFiles(subject: ReconcileSubject): Promise<ImportedFile[]> {
  const added = await invoke<ImportedFile[]>("reconcile_own_files", { subjectCode: subject.code });
  for (const file of added) {
    if (isPipelineFile(file.filename)) {
      parseFile(subject.id, subject.code, file.relative_path).catch((e) => console.warn(`[reconcile] parse ${file.relative_path}: ${e}`));
    }
  }
  if (added.length) window.dispatchEvent(new CustomEvent(UPLOADS_CHANGED_EVENT));
  await reconcileDocuments(subject);
  await reconcileWhiteboards(subject);
  await reconcileNotebooks(subject);
  return added;
}

interface Slot {
  subject: ReconcileSubject;
  timer: ReturnType<typeof setTimeout> | null;
  running: boolean;
  /** Asked again while running: one more pass once it ends. */
  again: boolean;
  force: boolean;
  lastRun: number;
}

const slots = new Map<number, Slot>();

/**
 * Reconcile `subject` soon: calls within `DEBOUNCE_MS` are one pass, never
 * two passes at once for a subject, and an unforced call this soon after the
 * last pass is dropped. A subject's Files tab and Drive's folders call it on
 * load; a finished chat turn forces it (`reconcileAfterTurn`).
 */
export function scheduleOwnReconcile(subject: ReconcileSubject, { force = false } = {}): void {
  let slot = slots.get(subject.id);
  if (!slot) {
    slot = { subject, timer: null, running: false, again: false, force: false, lastRun: 0 };
    slots.set(subject.id, slot);
  }
  slot.subject = subject;
  slot.force ||= force;
  if (!slot.force && Date.now() - slot.lastRun < COOLDOWN_MS) return;
  if (slot.running) {
    slot.again = true;
    return;
  }
  if (slot.timer) clearTimeout(slot.timer);
  const pending = slot;
  pending.timer = setTimeout(() => {
    pending.timer = null;
    void run(pending);
  }, DEBOUNCE_MS);
}

async function run(slot: Slot): Promise<void> {
  slot.running = true;
  slot.again = false;
  slot.force = false;
  try {
    await reconcileOwnFiles(slot.subject);
  } catch (e) {
    console.warn(`[reconcile] ${slot.subject.code}: ${e}`);
  } finally {
    slot.running = false;
    slot.lastRun = Date.now();
  }
  if (slot.again) {
    slot.again = false;
    slot.force = true;
    scheduleOwnReconcile(slot.subject);
  }
}

/** After a chat turn ends: the thread's subject, or every current subject for
 *  a thread filed under none, since the agent may have written anywhere. */
export async function reconcileAfterTurn(threadId: number): Promise<void> {
  try {
    const db = await getDb();
    const [filed] = await db.select<ReconcileSubject[]>(
      "SELECT s.id, s.code FROM harness_threads t JOIN subjects s ON s.id = t.subject_id WHERE t.id = $1",
      [threadId],
    );
    const subjects = filed ? [filed] : (await getSubjects()).filter((s) => s.is_current);
    for (const s of subjects) scheduleOwnReconcile({ id: s.id, code: s.code }, { force: true });
  } catch (e) {
    console.warn(`[reconcile] after thread ${threadId}: ${e}`);
  }
}
