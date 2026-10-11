import { useCallback, useEffect, useRef, useState } from "react";
import {
  getEmbedCoverage,
  getPipelineRows,
  getSubjects,
  getSyncRunSummaries,
  setParseStatusByPath,
  type Subject,
  type SyncRunSummary,
} from "@/lib/db";
import { embeddingStats } from "@/lib/pipeline/retrieval";
import { sqliteUtcToMs } from "@/lib/format/format";
import { scanParsedFiles } from "@/lib/files/courseFiles";
import { usePipelineStore } from "@/stores/sync/pipelineStore";
import { SEED_RETRY_MS } from "./constants";

/**
 * Loads the subjects and run table when the tab opens, keeps the run counts
 * live while a sync scrapes, and backfills the pipeline table from the DB.
 */
export function useSyncBoot(active: boolean, scraping: boolean) {
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [runs, setRuns] = useState<SyncRunSummary[]>([]);
  const seedPipeline = usePipelineStore((s) => s.seed);

  const loadFromDb = useCallback(async () => {
    const [rows, runRows] = await Promise.all([
      getSubjects(),
      getSyncRunSummaries(),
    ]);
    setSubjects(rows);
    setRuns(runRows);
    // Selection lives in the DB (`subjects.selected`).
    setSelectedIds(new Set(rows.filter((s) => s.selected).map((s) => s.id)));
  }, []);

  useEffect(() => {
    if (active) loadFromDb();
  }, [active, loadFromDb]);

  // While a run is scraping, keep the history table's counts live.
  useEffect(() => {
    if (!scraping || !active) return;
    const tick = () => getSyncRunSummaries().then(setRuns).catch(() => {});
    tick();
    const t = setInterval(tick, 2000);
    return () => clearInterval(t);
  }, [scraping, active]);

  // Backfill the pipeline table with every pipeline file on record. The DB's
  // parse_status can lag disk (e.g. CLI parses), so disk is consulted for
  // anything not fully parsed and the DB patched to match.
  const seedFromDb = useCallback(async () => {
    const rows = await getPipelineRows();
    const byPath = new Map(rows.map((r) => [r.relative_path, r]));

    // Seed embed from page coverage in the *current* space, never
    // `files.embed_status`, which doesn't know which model wrote the
    // vectors (same question as `getUnembeddedPdfs`).
    const { model, dim } = await embeddingStats();
    const coverage = new Map(
      (await getEmbedCoverage(model, dim)).map((c) => [c.relative_path, c]),
    );

    const unsure = rows
      .filter((r) => r.parse_status !== "quality")
      .map((r) => r.relative_path);
    let disk: Record<string, string> = {};
    if (unsure.length > 0) {
      const scanned = await scanParsedFiles(unsure);
      disk = Object.fromEntries(scanned);
      const fixes = scanned.filter(
        ([p, mode]) => mode !== (byPath.get(p)?.parse_status ?? ""),
      );
      if (fixes.length > 0) await setParseStatusByPath(fixes);
    }

    seedPipeline(
      rows.map((r) => ({
        relativePath: r.relative_path,
        subjectId: r.subject_id,
        parseStatus: disk[r.relative_path] ?? r.parse_status,
        embedStatus: r.embed_status,
        pagesTotal: coverage.get(r.relative_path)?.pages_total ?? 0,
        pagesCurrent: coverage.get(r.relative_path)?.pages_current ?? 0,
        // `scraped_at` moves on every sync, changed bytes or not.
        downloadedAt: sqliteUtcToMs(
          r.content_changed_at ?? r.first_seen_at ?? r.scraped_at,
        ),
        parsedAt: sqliteUtcToMs(r.parsed_at),
        embeddedAt: sqliteUtcToMs(r.embedded_at),
      })),
    );
  }, [seedPipeline]);

  // Re-seeded on every activation and after every sync run, since the CLI,
  // deletes and renames change the DB without a live event. One seed at a
  // time: a request while one runs queues a single rerun.
  const seedRun = useRef({ running: false, again: false });
  const seedRetry = useRef<ReturnType<typeof setTimeout> | null>(null);
  const activeRef = useRef(active);
  activeRef.current = active;

  const requestSeed = useCallback(function request() {
    const run = seedRun.current;
    if (run.running) {
      run.again = true;
      return;
    }
    run.running = true;
    if (seedRetry.current) clearTimeout(seedRetry.current);
    seedRetry.current = null;
    seedFromDb()
      .then(
        () => false,
        (e) => {
          console.error("pipeline seed failed", e);
          return true;
        },
      )
      .then((failed) => {
        run.running = false;
        if (run.again) {
          run.again = false;
          request();
        } else if (failed && activeRef.current) {
          seedRetry.current = setTimeout(request, SEED_RETRY_MS);
        }
      });
  }, [seedFromDb]);

  useEffect(() => {
    if (active) requestSeed();
  }, [active, requestSeed]);

  useEffect(() => () => {
    if (seedRetry.current) clearTimeout(seedRetry.current);
  }, []);

  return { subjects, selectedIds, setSelectedIds, runs, setRuns, loadFromDb, requestSeed, activeRef };
}
