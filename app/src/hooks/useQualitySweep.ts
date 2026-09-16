import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getDb } from "@/lib/db";
import { useParseStore } from "@/stores/parseStore";

/**
 * Background parse sweep — the recovery path for files that missed their
 * parse (app closed mid-queue, a parse died, a sync ran while parsing was
 * down). Every so often it re-requests a few PDF-backed files in selected
 * subjects that are not at `quality` yet; the Rust side skips whatever
 * already exists.
 *
 * The name is historical, like the `quality` status itself: there is one
 * parse tier now, and `'quality'` is simply what a finished parse is called
 * in the DB.
 *
 * ## Why this is careful now
 *
 * It used to be free to be wrong. A failure fell back to a local parser, so
 * re-kicking a permanently broken file cost a little CPU. **Every parse is
 * now a metered cloud call.** Left as it was, this loop would resubmit every
 * permanently-failed file — a corrupt PDF, one over the size limit, one
 * rejected outright — every 15 minutes forever, and a bad token or an
 * exhausted quota would march the entire library through the same error one
 * batch at a time, burning the day's allowance on parses that cannot succeed.
 *
 * So two gates, both reading the discriminants the `parse-status` error
 * carries (`parseStore`):
 *
 * 1. **`retryable === false` is never re-kicked.** The failure said retrying
 *    this file can never work; asking again is pure spend. An *unknown*
 *    retryability (a file that failed in a previous session, where all the DB
 *    keeps is the word `error`) is still eligible — otherwise the recovery
 *    path this hook exists to be would die — so a permanently bad file costs
 *    at most one attempt per app launch instead of four an hour.
 * 2. **A latching failure stands the sweep down.** No token, a rejected
 *    token, exhausted quota: the next file would fail identically, so the
 *    sweep stops rather than proving it 8 files at a time. The latch lifts
 *    the moment any parse gets anywhere (`parseStore.update`), which includes
 *    every hand-driven route — a sync, the pipeline row's retry — so the user
 *    fixing the cause is what resumes it. `LATCH_PROBE_AFTER_MS` is the
 *    safety valve for the one condition that heals on a clock rather than on
 *    an action: a daily quota. After it, the sweep spends exactly one file to
 *    ask whether the condition still holds.
 *
 * If this ever looks like over-engineering worth simplifying: the simple
 * version is a quota fire.
 */
const FIRST_SWEEP_DELAY_MS = 90 * 1000;
const SWEEP_INTERVAL_MS = 15 * 60 * 1000;
/** Per sweep, so a fresh install drains gradually instead of flooding. */
const MAX_KICKS_PER_SWEEP = 8;
/** How long a latching failure silences the sweep before it spends one file
 *  finding out whether the condition has lifted. */
const LATCH_PROBE_AFTER_MS = 6 * 60 * 60 * 1000;

async function sweep(): Promise<void> {
  const { statuses: live, failures, latch } = useParseStore.getState();

  // Gate 2: a condemning failure is in force.
  let budget = MAX_KICKS_PER_SWEEP;
  if (latch) {
    if (Date.now() - latch.at < LATCH_PROBE_AFTER_MS) {
      console.info(`[parse-sweep] standing down — ${latch.kind ?? "parsing unavailable"}`);
      return;
    }
    budget = 1; // one probe, not a batch
  }

  const db = await getDb();
  const rows = await db.select<
    { subject_id: number; relative_path: string; code: string }[]
  >(
    `SELECT f.subject_id, f.relative_path, s.code
     FROM files f JOIN subjects s ON s.id = f.subject_id
     WHERE s.selected = 1
       AND lower(f.file_type) IN ('pdf', 'pptx', 'docx', 'ppt', 'doc')
       AND (f.parse_status IS NULL OR f.parse_status != 'quality')
     ORDER BY f.scraped_at DESC`,
  );
  if (rows.length === 0) return;

  const outstanding = rows.filter((r) => {
    // Live in this session already — don't queue the same file twice. Stale
    // DB "queued"/"running" rows from a previous session have no live entry
    // and do get re-kicked, which is the case this sweep exists for.
    if (["queued", "running"].includes(live[r.relative_path] ?? "")) return false;
    // Gate 1: this file's last failure said a retry cannot work.
    if (failures[r.relative_path]?.retryable === false) return false;
    return true;
  });
  if (outstanding.length === 0) return;

  const kicking = Math.min(outstanding.length, budget);
  console.info(
    `[parse-sweep] ${outstanding.length} file(s) unparsed, kicking ${kicking}${latch ? " (probe)" : ""}`,
  );
  for (const r of outstanding.slice(0, budget)) {
    try {
      await invoke("parse_file", {
        subjectId: r.subject_id,
        subjectCode: r.code,
        relativePath: r.relative_path,
      });
    } catch (e) {
      console.warn(`[parse-sweep] ${r.relative_path}: ${e}`);
    }
  }
}

/** Mount once at the app root. */
export function useQualitySweep(): void {
  useEffect(() => {
    const run = () => sweep().catch((e) => console.warn("[parse-sweep]", e));
    const first = setTimeout(run, FIRST_SWEEP_DELAY_MS);
    const every = setInterval(run, SWEEP_INTERVAL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(every);
    };
  }, []);
}
