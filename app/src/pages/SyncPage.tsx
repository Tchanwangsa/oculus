import { useEffect, useState, useCallback, useRef, useMemo } from "react";
import {
  WarningCircle,
  XCircle,
  Broom,
  Play,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  getDb,
  upsertSubjects,
  getSubjects,
  getSyncRunSummaries,
  getPdfPipelineRows,
  setParseStatusByPath,
  setSubjectSelected,
  addLog,
  type CanvasCourseRaw,
  type Subject,
  type SyncRunSummary,
} from "@/lib/db";
import { embedFile, embedPending } from "@/lib/retrieval";
import { triggerSync } from "@/lib/syncRunner";
import { fmtAgo, sqliteUtcToMs } from "@/lib/format";
import { useAuth } from "@/hooks/useAuth";
import { useSyncStore } from "@/stores/syncStore";
import {
  usePipelineStore,
  isComplete,
  hasFailed,
  statusOf,
  type PipelineItem,
} from "@/stores/pipelineStore";
import { PipelineTable } from "@/components/sync/PipelineTable";
import { SyncHistoryTable } from "@/components/sync/SyncHistoryTable";
import { SubjectPicker } from "@/components/sync/SubjectPicker";
import { SyncSettings } from "@/components/sync/SyncSettings";

const PHASE_LABEL: Record<string, string> = {
  home: "overview",
  announcements: "announcements",
  modules: "modules",
};

type ActivityView = "history" | "pipeline";
const VIEW_KEY = "oculus-sync-view";

export default function SyncPage() {
  const { status: authStatus, connect } = useAuth();
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [loadingSubjects, setLoadingSubjects] = useState(false);
  const [subjectsError, setSubjectsError] = useState<string | null>(null);
  const [runs, setRuns] = useState<SyncRunSummary[]>([]);
  const [view, setView] = useState<ActivityView>(() =>
    localStorage.getItem(VIEW_KEY) === "pipeline" ? "pipeline" : "history",
  );

  useEffect(() => {
    localStorage.setItem(VIEW_KEY, view);
  }, [view]);

  // Sync progress lives in the global store (survives navigation).
  const scraping = useSyncStore((s) => s.scraping);
  const progress = useSyncStore((s) => s.progress);
  const completedAt = useSyncStore((s) => s.completedAt);
  const syncError = useSyncStore((s) => s.error);
  const scrapingRef = useRef(false);

  // Pipeline (per-file download → parse → embed tracking).
  const pipelineItems = usePipelineStore((s) => s.items);
  const seedPipeline = usePipelineStore((s) => s.seed);
  const clearFinished = usePipelineStore((s) => s.clearFinished);

  useEffect(() => {
    scrapingRef.current = scraping;
  }, [scraping]);

  // ── Boot ──────────────────────────────────────────────────────────────────

  const loadFromDb = useCallback(async () => {
    const [rows, runRows] = await Promise.all([
      getSubjects(),
      getSyncRunSummaries(),
    ]);
    setSubjects(rows);
    setRuns(runRows);
    // Selection lives in the DB (subjects.selected), so it survives leaving
    // the tab and app restarts.
    setSelectedIds(new Set(rows.filter((s) => s.selected).map((s) => s.id)));
  }, []);

  useEffect(() => {
    loadFromDb();
  }, [loadFromDb]);

  // While a run is scraping, keep the history table's counts live.
  useEffect(() => {
    if (!scraping) return;
    const tick = () => getSyncRunSummaries().then(setRuns).catch(() => {});
    tick();
    const t = setInterval(tick, 2000);
    return () => clearInterval(t);
  }, [scraping]);

  // Backfill the pipeline table with every PDF on record, so the backlog
  // (awaiting parse, awaiting embed) is visible even before anything runs.
  // The DB's parse_status lags reality for files parsed before status
  // tracking existed (or by the CLI), so disk is consulted for anything the
  // DB doesn't already call fully parsed — and the DB is patched to match.
  useEffect(() => {
    (async () => {
      try {
        const rows = await getPdfPipelineRows();
        const byPath = new Map(rows.map((r) => [r.relative_path, r]));

        const unsure = rows
          .filter((r) => r.parse_status !== "quality")
          .map((r) => r.relative_path);
        let disk: Record<string, string> = {};
        if (unsure.length > 0) {
          const scanned = await invoke<[string, string][]>("scan_parsed_files", {
            relativePaths: unsure,
          });
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
            downloadedAt: sqliteUtcToMs(r.scraped_at),
            parsedAt: sqliteUtcToMs(r.parsed_at),
            embeddedAt: sqliteUtcToMs(r.embedded_at),
          })),
        );
      } catch (e) {
        console.error("pipeline seed failed", e);
      }
    })();
  }, [seedPipeline]);

  // ── Derived pipeline counts ───────────────────────────────────────────────

  const items = useMemo(() => Object.values(pipelineItems), [pipelineItems]);
  const counts = useMemo(() => {
    let active = 0, waiting = 0, paused = 0, failed = 0, done = 0;
    for (const it of items) {
      const phase = statusOf(it).phase;
      if (phase === "active") active++;
      else if (phase === "waiting") waiting++;
      else if (phase === "paused") paused++;
      else if (phase === "failed") failed++;
      else done++;
    }
    return { active, waiting, paused, failed, done };
  }, [items]);

  // ── Auth events (cancelled, expired) ──────────────────────────────────────

  useEffect(() => {
    const sub = listen("canvas-auth-expired", () => {
      if (scrapingRef.current) {
        useSyncStore.getState().reset();
        setSubjectsError(
          "Canvas session expired during sync. Reconnect and try again.",
        );
      }
    });
    return () => {
      sub.then((f) => f());
    };
  }, []);

  // ── Subjects events ───────────────────────────────────────────────────────

  useEffect(() => {
    const subs = [
      listen<CanvasCourseRaw[]>("subjects-loaded", async (e) => {
        setLoadingSubjects(false);
        setSubjectsError(null);
        try {
          await upsertSubjects(e.payload as unknown as CanvasCourseRaw[]);
          await addLog(`Fetched ${e.payload.length} subjects from Canvas`);
          await loadFromDb();
        } catch (err) {
          console.error("DB upsert failed:", err);
        }
      }),
      listen<string>("subjects-error", (e) => {
        setSubjectsError(e.payload);
        setLoadingSubjects(false);
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((f) => f()));
    };
  }, [loadFromDb]);

  // Refresh subjects and the run table when a sync run finishes.
  useEffect(() => {
    if (completedAt === 0) return;
    if (syncError) {
      setSubjectsError(`Sync error: ${syncError}`);
    }
    loadFromDb();
  }, [completedAt, syncError, loadFromDb]);

  // ── Handlers ──────────────────────────────────────────────────────────────

  const handleRefetchSubjects = async () => {
    setLoadingSubjects(true);
    setSubjectsError(null);
    try {
      await invoke("sync_subjects");
    } catch (err) {
      setLoadingSubjects(false);
      setSubjectsError(String(err));
    }
  };

  const handleCancel = async () => {
    try {
      await invoke("cancel_scrape");
    } catch {
      /* ignore */
    }
  };

  const handleSyncClick = async () => {
    // No live session? The click becomes the reauth: open the Canvas sign-in
    // window instead of failing. A sync can start once it reports success.
    if (authStatus !== "connected") {
      setSubjectsError(null);
      connect();
      return;
    }
    if (selectedIds.size === 0) return;

    setSubjectsError(null);
    setView("history");

    try {
      await triggerSync("manual");
      await getSyncRunSummaries().then(setRuns);
    } catch (err) {
      setSubjectsError(String(err));
    }
  };

  /**
   * Pick the pipeline back up for one file, from whichever stage is
   * outstanding. Parsing is idempotent on the sidecar side (fast is skipped
   * when markdown exists, quality when its record exists), so "resume" and
   * "retry" are the same call; a file that only lacks its embed goes straight
   * to the embedder.
   */
  const resumeItem = useCallback(async (it: PipelineItem) => {
    const { touch } = usePipelineStore.getState();
    // Clear paused/failed immediately so the row reads as moving again.
    touch(it.relativePath, it.subjectId, {
      ...(it.quality === "error" ? { quality: "pending" as const } : {}),
      ...(it.embed === "error" ? { embed: "pending" as const } : {}),
      error: undefined,
    });
    try {
      if (it.quality !== "done") {
        await invoke("parse_file", {
          subjectId: it.subjectId,
          subjectCode: it.code,
          relativePath: it.relativePath,
        });
      } else if (it.embed !== "done") {
        const db = await getDb();
        const rows = await db.select<{ id: number }[]>(
          `SELECT id FROM files WHERE subject_id = $1 AND relative_path = $2`,
          [it.subjectId, it.relativePath],
        );
        const fileId = rows[0]?.id;
        if (fileId == null) throw new Error("file not in the database");
        await embedFile(fileId, it.relativePath);
      }
    } catch (e) {
      touch(
        it.relativePath,
        it.subjectId,
        it.quality !== "done"
          ? { quality: "error", error: String(e) }
          : { embed: "error", error: String(e) },
      );
    }
  }, []);

  /** Resume every paused row. Parses queue up in the sidecar; files that only
   *  need an embed run through `embedPending`, which is serialised already. */
  const resumeAll = useCallback(() => {
    const all = Object.values(usePipelineStore.getState().items);
    const { touch } = usePipelineStore.getState();
    let needEmbed = false;
    for (const it of all) {
      if (statusOf(it).phase !== "paused") continue;
      if (it.quality !== "done") {
        void resumeItem(it);
      } else if (it.embed !== "done") {
        touch(it.relativePath, it.subjectId, {});
        needEmbed = true;
      }
    }
    if (needEmbed) embedPending().catch((e) => console.error("resume embeds failed", e));
  }, [resumeItem]);

  const toggleSubject = (id: number) => {
    const nowSelected = !selectedIds.has(id);
    setSelectedIds((prev) => {
      const next = new Set(prev);
      nowSelected ? next.add(id) : next.delete(id);
      return next;
    });
    setSubjectSelected(id, nowSelected).catch((e) =>
      console.error("persist subject selection failed", e),
    );
  };

  // ── Derived ───────────────────────────────────────────────────────────────

  const phase = progress?.phase ? (PHASE_LABEL[progress.phase] ?? progress.phase) : null;
  const finishedCount = items.filter((it) => isComplete(it) || hasFailed(it)).length;
  const lastCompleted = runs.find((r) => r.status === "completed" && r.finished_at);
  const needsAuth = authStatus !== "connected";

  return (
    <div className="flex flex-col h-full">
      {/* ── Header: activity view switcher + per-view actions ────────────── */}
      <div className="shrink-0 flex items-center gap-2 px-6 pt-5 pb-3">
        <Select value={view} onValueChange={(v) => setView(v as ActivityView)}>
          <SelectTrigger
            size="sm"
            className="h-7 -ml-2 gap-1.5 border-0 bg-transparent shadow-none px-2 text-[13px] font-medium text-foreground hover:bg-surface dark:bg-transparent dark:hover:bg-surface"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="history">Sync History</SelectItem>
            <SelectItem value="pipeline">Parse Activity</SelectItem>
          </SelectContent>
        </Select>

        {view === "pipeline" && (
          <div className="flex items-center gap-1.5 ml-1">
            {counts.active > 0 && (
              <Badge className="text-[11px]">{counts.active} running</Badge>
            )}
            {counts.waiting > 0 && (
              <Badge variant="secondary" className="text-[11px]">
                {counts.waiting} waiting
              </Badge>
            )}
            {counts.paused > 0 && (
              <Badge variant="warning" className="text-[11px]">
                {counts.paused} paused
              </Badge>
            )}
            {counts.failed > 0 && (
              <Badge variant="destructive" className="text-[11px]">
                {counts.failed} failed
              </Badge>
            )}
            {counts.done > 0 && (
              <Badge variant="success" className="text-[11px]">
                {counts.done} done
              </Badge>
            )}
          </div>
        )}

        <div className="ml-auto flex items-center gap-1">
          {view === "pipeline" && counts.paused > 0 && (
            <Button
              variant="ghost"
              size="sm"
              onClick={resumeAll}
              className="h-7 text-xs text-muted-foreground hover:text-foreground"
            >
              <Play size={13} /> Resume all ({counts.paused})
            </Button>
          )}
          {view === "pipeline" && finishedCount > 0 && (
            <Button
              variant="ghost"
              size="sm"
              onClick={clearFinished}
              className="h-7 text-xs text-muted-foreground hover:text-foreground"
            >
              <Broom size={13} /> Clear finished
            </Button>
          )}
        </div>
      </div>

      {/* ── Sync controls: subjects + status + run, above the table ──────── */}
      {view === "history" && (
        <div className="shrink-0 flex items-center gap-3 px-6 pb-3">
          <SubjectPicker
            subjects={subjects}
            selectedIds={selectedIds}
            onToggle={toggleSubject}
            onRefetch={handleRefetchSubjects}
            refetching={loadingSubjects}
            canRefetch={authStatus === "connected"}
          />

          <span className="flex-1" />

          {scraping && progress ? (
            <div className="flex items-center gap-2.5 min-w-0">
              <span className="text-[11px] text-muted-foreground truncate max-w-64">
                {progress.course
                  ? `${progress.course}${phase && progress.phase !== "complete" ? ` — ${phase}` : ""}`
                  : "Starting…"}
              </span>
              <Progress
                value={progress.total ? (progress.done / progress.total) * 100 : 0}
                className="h-1 w-24"
              />
              <span className="text-[11px] text-muted-foreground tabular-nums shrink-0">
                {progress.done}/{progress.total}
              </span>
            </div>
          ) : (
            <span className="text-[11px] text-muted-foreground shrink-0">
              Last synced{" "}
              {lastCompleted ? fmtAgo(sqliteUtcToMs(lastCompleted.finished_at)) : "never"}
            </span>
          )}

          <SyncSettings />

          {scraping ? (
            <Button
              variant="outline"
              size="sm"
              onClick={handleCancel}
              className="h-7 shrink-0 text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
            >
              <XCircle size={13} /> Cancel
            </Button>
          ) : needsAuth || selectedIds.size === 0 ? (
            <Tooltip>
              {/* span wrapper: a disabled button swallows pointer events, so
                  the tooltip must hang off something that still gets them. */}
              <TooltipTrigger asChild>
                <span className="shrink-0">
                  <Button
                    size="sm"
                    className={cn("h-7", needsAuth && "opacity-50")}
                    onClick={handleSyncClick}
                    disabled={selectedIds.size === 0}
                  >
                    Sync now
                  </Button>
                </span>
              </TooltipTrigger>
              <TooltipContent>
                {needsAuth
                  ? authStatus === "expired"
                    ? "Canvas session expired — click to sign in again"
                    : "Not connected to Canvas — click to sign in"
                  : "Select at least one subject first"}
              </TooltipContent>
            </Tooltip>
          ) : (
            <Button size="sm" className="h-7 shrink-0" onClick={handleSyncClick}>
              Sync now
            </Button>
          )}
        </div>
      )}

      {/* ── Body: the selected table ─────────────────────────────────────── */}
      <div className="flex-1 min-h-0 overflow-y-auto px-6 pb-5">
        {view === "history" ? (
          <SyncHistoryTable runs={runs} progress={progress} />
        ) : (
          <PipelineTable items={items} onResume={resumeItem} />
        )}
      </div>

      {subjectsError && (
        <div className="shrink-0 px-6 pb-3">
          <Alert variant="destructive" className="px-3 py-2.5">
            <WarningCircle />
            <AlertDescription className="text-xs">{subjectsError}</AlertDescription>
          </Alert>
        </div>
      )}
    </div>
  );
}
