import { useEffect } from "react";
import { getDb } from "@/lib/db";
import { PDF_BACKED_SQL_LIST } from "@/lib/fileTypes";
import { useParseStore } from "@/stores/parseStore";
import { parseFile } from "@/lib/courseFiles";

/**
 * Background parse sweep: the recovery path for files that missed their parse
 * (app closed mid-queue, a parse died, a sync ran while parsing was down).
 * Periodically re-requests a few PDF-backed files in selected subjects that
 * are not at `quality` (the DB's name for a finished parse); Rust skips what
 * already exists.
 *
 * A parse is never free — a metered cloud call, or minutes of this machine's
 * CPU on a local MinerU — and nothing catches a failure (docs/parsing.md), so two
 * gates read the `parse-status` error's discriminants (`parseStore`):
 * 1. `retryable === false` is never re-kicked. Unknown retryability (a
 *    failure from a previous session) stays eligible, so a bad file costs at
 *    most one attempt per launch.
 * 2. A latching failure (no token, rejected token, spent quota) stands the
 *    sweep down until any parse progresses (`parseStore.update`), or until
 *    `LATCH_PROBE_AFTER_MS`, when it spends one file to probe a daily quota.
 */
const FIRST_SWEEP_DELAY_MS = 90 * 1000;
const SWEEP_INTERVAL_MS = 15 * 60 * 1000;
/** Per sweep, so a fresh install drains gradually instead of flooding. */
const MAX_KICKS_PER_SWEEP = 8;
/** How long a latch silences the sweep before one probe file. */
const LATCH_PROBE_AFTER_MS = 6 * 60 * 60 * 1000;

async function sweep(): Promise<void> {
  const { statuses: live, failures, latch } = useParseStore.getState();

  // Gate 2.
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
       AND lower(f.file_type) IN ${PDF_BACKED_SQL_LIST}
       AND (f.parse_status IS NULL OR f.parse_status != 'quality')
     ORDER BY f.scraped_at DESC`,
  );
  if (rows.length === 0) return;

  const outstanding = rows.filter((r) => {
    // Skip files live in this session; stale DB "queued"/"running" rows from
    // a previous session have no live entry and are re-kicked.
    if (["queued", "running"].includes(live[r.relative_path] ?? "")) return false;
    // Gate 1.
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
      await parseFile(r.subject_id, r.code, r.relative_path);
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
