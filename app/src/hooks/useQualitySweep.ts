import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getDb } from "@/lib/db";
import { useParseStore } from "@/stores/parseStore";

/**
 * Background quality-parse sweep.
 *
 * Files can miss their quality pass — the app closed while they sat in the
 * sidecar's queue, the sidecar was down during a sync, a parse died midway.
 * Every so often, re-request the pipeline for any PDF-backed file in a
 * selected subject that isn't at "quality" yet; the sidecar skips whatever
 * already exists and queues only the missing pass.
 */
const FIRST_SWEEP_DELAY_MS = 90 * 1000; // let the sidecar finish warming up
const SWEEP_INTERVAL_MS = 15 * 60 * 1000;
/** Per sweep, so a fresh install drains gradually instead of flooding the queue. */
const MAX_KICKS_PER_SWEEP = 8;

async function sweep(): Promise<void> {
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

  // Live in this session already — don't queue the same file twice. Stale DB
  // "queued"/"running" rows from a previous session have no live entry and do
  // get re-kicked, which is exactly the case the sweep exists for.
  const live = useParseStore.getState().statuses;
  const outstanding = rows.filter(
    (r) => !["queued", "running"].includes(live[r.relative_path] ?? ""),
  );
  if (outstanding.length === 0) return;

  console.info(
    `[quality-sweep] ${outstanding.length} file(s) without a quality parse, kicking ${Math.min(outstanding.length, MAX_KICKS_PER_SWEEP)}`,
  );
  for (const r of outstanding.slice(0, MAX_KICKS_PER_SWEEP)) {
    try {
      await invoke("parse_file", {
        subjectId: r.subject_id,
        subjectCode: r.code,
        relativePath: r.relative_path,
      });
    } catch (e) {
      console.warn(`[quality-sweep] ${r.relative_path}: ${e}`);
    }
  }
}

/** Mount once at the app root. */
export function useQualitySweep(): void {
  useEffect(() => {
    const run = () => sweep().catch((e) => console.warn("[quality-sweep]", e));
    const first = setTimeout(run, FIRST_SWEEP_DELAY_MS);
    const every = setInterval(run, SWEEP_INTERVAL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(every);
    };
  }, []);
}
