import { useEffect, useState, useCallback, useRef } from "react";
import {
  RefreshCw, AlertCircle,
  BookOpen, ChevronRight, ChevronDown,
  Clock, Loader2, XCircle, Settings, Bug,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogFooter } from "@/components/ui/dialog";
import {
  upsertSubjects, getSubjects, getLastCompletedSyncRun, addLog,
  startSyncRun, finishSyncRun, markSubjectSynced, upsertFile,
  type CanvasCourseRaw, type Subject, type SyncRun,
} from "@/lib/db";
import { fmtDate } from "@/lib/format";
import { useAuth, type AuthStatus } from "@/hooks/useAuth";
import { AuthCard } from "@/components/sync/AuthCard";
import { PipelineSteps } from "@/components/sync/PipelineSteps";
import { SubjectRow } from "@/components/subjects/SubjectRow";

// ── Types ─────────────────────────────────────────────────────────────────────

type StepStatus = "done" | "pending" | "error" | "idle";
type PipelineStep = { id: string; status: StepStatus };

const INITIAL_STEPS: PipelineStep[] = [
  { id: "auth",    status: "idle" },
  { id: "courses", status: "idle" },
  { id: "scrape",  status: "idle" },
  { id: "graph",   status: "idle" },
];

const AUTH_BADGE: Record<AuthStatus, { label: string; variant: "success" | "secondary" | "warning" }> = {
  connected:    { label: "Connected",    variant: "success"   },
  disconnected: { label: "Disconnected", variant: "secondary" },
  pending:      { label: "Signing in…",  variant: "warning"   },
};

// ── Component ─────────────────────────────────────────────────────────────────

export default function SyncPage() {
  const { status: authStatus } = useAuth();
  const [steps, setSteps] = useState(INITIAL_STEPS);
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [showModal, setShowModal] = useState(false);
  const [loadingSubjects, setLoadingSubjects] = useState(false);
  const [subjectsError, setSubjectsError] = useState<string | null>(null);
  const [lastSyncRun, setLastSyncRun] = useState<SyncRun | null>(null);
  const [pastExpanded, setPastExpanded] = useState(false);
  const [scraping, setScraping] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number; course?: string; phase?: string } | null>(null);
  const runIdRef = useRef<number | null>(null);
  const scrapingRef = useRef(false);

  useEffect(() => { scrapingRef.current = scraping; }, [scraping]);

  // ── Boot ──────────────────────────────────────────────────────────────────

  const loadFromDb = useCallback(async () => {
    const [rows, lastRun] = await Promise.all([
      getSubjects(),
      getLastCompletedSyncRun(),
    ]);
    setSubjects(rows);
    setLastSyncRun(lastRun);
    setSelectedIds((prev) => {
      const next = new Set(prev);
      rows.filter((s) => s.is_current).forEach((s) => next.add(s.id));
      return next;
    });
  }, []);

  useEffect(() => {
    loadFromDb();
  }, [loadFromDb]);

  // Sync auth status with pipeline steps
  useEffect(() => {
    if (authStatus === "connected") {
      setSteps((p) => p.map((s) => (s.id === "auth" ? { ...s, status: "done" } : s)));
    }
  }, [authStatus]);

  // ── Auth events (cancelled, expired) ──────────────────────────────────────

  useEffect(() => {
    const subs = [
      listen("canvas-auth-cancelled", () => {
        // useAuth handles status; no-op here
      }),
      listen("canvas-auth-expired", () => {
        setSteps(INITIAL_STEPS);
        if (scrapingRef.current) {
          setScraping(false);
          setProgress(null);
          setSubjectsError("Canvas session expired during sync. Reconnect and try again.");
        }
      }),
    ];
    return () => { subs.forEach((p) => p.then((f) => f())); };
  }, []);

  // ── Subjects events ───────────────────────────────────────────────────────

  useEffect(() => {
    const subs = [
      listen<CanvasCourseRaw[]>("subjects-loaded", async (e) => {
        setLoadingSubjects(false);
        setSubjectsError(null);
        setSteps((p) => p.map((s) => (s.id === "courses" ? { ...s, status: "done" } : s)));
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
        setSteps((p) => p.map((s) => (s.id === "courses" ? { ...s, status: "error" } : s)));
      }),
    ];
    return () => { subs.forEach((p) => p.then((f) => f())); };
  }, [loadFromDb]);

  // ── Scrape events ─────────────────────────────────────────────────────────

  useEffect(() => {
    const subs = [
      listen<{ subject_id: number; relative_path: string; size_bytes: number; category: string | null; canvas_id: number | null }>(
        "scrape-file",
        async (e) => {
          const { subject_id, relative_path, size_bytes, category, canvas_id } = e.payload;
          const filename = relative_path.split("/").pop() ?? relative_path;
          const ext = filename.includes(".") ? filename.split(".").pop()! : "md";
          try {
            await upsertFile(subject_id, filename, relative_path, ext, size_bytes, category ?? undefined, canvas_id ?? undefined);
            await markSubjectSynced(subject_id);
          } catch { /* ignore */ }
        }
      ),
      listen<{ done: number; total: number; course?: string; phase?: string }>("scrape-progress", (e) => {
        setProgress(e.payload);
      }),
      listen<{ level: string; course: string; message: string }>("scrape-log", (e) => {
        const { level, message } = e.payload;
        const mapped = level === "error" ? "error" : level === "warning" ? "warning" : "info";
        addLog(message, mapped).catch(() => {});
      }),
      listen<{ count: number; cancelled?: boolean }>("scrape-complete", async (e) => {
        const runId = runIdRef.current;
        if (runId != null) {
          await finishSyncRun(runId, "completed", e.payload.count, e.payload.count);
          await addLog(`Synced ${e.payload.count} subject(s)`);
        }
        setScraping(false);
        setProgress(null);
        setSteps((p) => p.map((s) => (s.id === "scrape" ? { ...s, status: "done" } : s)));
        await loadFromDb();
      }),
      listen<string>("scrape-error", async (e) => {
        const runId = runIdRef.current;
        if (runId != null) await finishSyncRun(runId, "failed", 0, 0, e.payload);
        setScraping(false);
        setProgress(null);
        setSubjectsError(`Scrape error: ${e.payload}`);
        setSteps((p) => p.map((s) => (s.id === "scrape" ? { ...s, status: "error" } : s)));
      }),
    ];
    return () => { subs.forEach((p) => p.then((f) => f())); };
  }, [loadFromDb]);

  // ── Handlers ──────────────────────────────────────────────────────────────

  const handleRefetchSubjects = async () => {
    setLoadingSubjects(true);
    setSubjectsError(null);
    setSteps((p) => p.map((s) => (s.id === "courses" ? { ...s, status: "pending" } : s)));
    try {
      await invoke("sync_subjects");
    } catch (err) {
      setLoadingSubjects(false);
      setSubjectsError(String(err));
      setSteps((p) => p.map((s) => (s.id === "courses" ? { ...s, status: "error" } : s)));
    }
  };

  const handleCancel = async () => {
    try { await invoke("cancel_scrape"); } catch { /* ignore */ }
  };

  const handleSyncClick = async () => {
    if (selectedIds.size === 0 || subjects.length === 0) {
      setShowModal(true);
      return;
    }
    if (authStatus !== "connected") {
      setSubjectsError("Not connected to Canvas. Connect first.");
      return;
    }
    const sel = subjects
      .filter((s) => selectedIds.has(s.id))
      .map((s) => ({ id: s.id, code: s.code }));

    setScraping(true);
    setSubjectsError(null);
    setProgress({ done: 0, total: sel.length });
    setSteps((p) => p.map((s) => (s.id === "scrape" ? { ...s, status: "pending" } : s)));

    try {
      runIdRef.current = await startSyncRun();
      await invoke("scrape_content", { subjects: sel });
    } catch (err) {
      setScraping(false);
      setProgress(null);
      setSubjectsError(String(err));
      setSteps((p) => p.map((s) => (s.id === "scrape" ? { ...s, status: "error" } : s)));
      if (runIdRef.current != null) await finishSyncRun(runIdRef.current, "failed", 0, 0, String(err));
    }
  };

  const toggleSubject = (id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  };

  // ── Derived ───────────────────────────────────────────────────────────────

  const authBadge = AUTH_BADGE[authStatus];
  const current = subjects.filter((s) => s.is_current);
  const past = subjects.filter((s) => !s.is_current);
  const noSubjects = subjects.length === 0;

  const pastBySemester = past.reduce<Record<string, Subject[]>>((acc, s) => {
    const key = s.term_name ?? "Unknown term";
    (acc[key] ??= []).push(s);
    return acc;
  }, {});

  return (
    <div className="flex flex-col h-full overflow-y-auto">
      {/* Header */}
      <div className="px-6 h-14 flex items-center gap-3 border-b border-border shrink-0">
        <RefreshCw size={16} className="text-primary" />
        <span className="font-semibold text-foreground">Sync</span>
        <Badge variant={authBadge.variant}>{authBadge.label}</Badge>
      </div>

      <div className="px-6 py-6 space-y-5 max-w-2xl">
        {/* Auth card */}
        <AuthCard
          status={authStatus}
          onConnect={async () => {
            try { await invoke("launch_canvas_auth"); } catch { /* useAuth handles */ }
          }}
          onDisconnect={async () => {
            setSteps(INITIAL_STEPS);
            setSubjects([]);
            setSelectedIds(new Set());
            try { await invoke("disconnect_canvas"); } catch { /* ignore */ }
          }}
          scraping={scraping}
        />

        {/* Sync card */}
        <div className="rounded-xl border border-border bg-card p-5">
          <div className="flex items-center justify-between mb-1">
            <span className="font-semibold text-sm text-foreground">Content Sync</span>
            <button className="text-muted-foreground hover:text-foreground transition-colors" title="Sync settings (coming soon)">
              <Settings size={14} />
            </button>
          </div>
          <p className="text-xs text-muted-foreground mb-4 flex items-center gap-1.5">
            <Clock size={11} />
            Last synced: {fmtDate(lastSyncRun?.finished_at ?? null)}
          </p>

          <div className="h-px bg-border mb-4" />

          <div className="flex gap-2">
            <Button
              className="flex-1 gap-2"
              onClick={handleSyncClick}
              disabled={authStatus !== "connected" || scraping}
            >
              {scraping ? (
                <><Loader2 size={14} className="animate-spin" /> Syncing…</>
              ) : (
                <>Sync ({selectedIds.size})</>
              )}
            </Button>

            {scraping ? (
              <Button
                variant="outline"
                size="icon-sm"
                title="Cancel sync"
                onClick={handleCancel}
                className="shrink-0 w-9 h-9 text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
              >
                <XCircle size={14} />
              </Button>
            ) : (
              <Button
                variant="outline"
                size="icon-sm"
                title="Manage subjects"
                onClick={() => setShowModal(true)}
                className="shrink-0 w-9 h-9"
                disabled={scraping}
              >
                <BookOpen size={14} />
              </Button>
            )}

            <Button
              variant="ghost"
              size="icon-sm"
              title="Open Canvas WebView DevTools"
              onClick={() => invoke("open_canvas_devtools")}
              className="shrink-0 w-9 h-9 text-muted-foreground"
            >
              <Bug size={13} />
            </Button>
          </div>

          {noSubjects && authStatus === "connected" && (
            <p className="text-xs text-muted-foreground mt-2 text-center">
              No subjects loaded —{" "}
              <button className="text-primary hover:underline" onClick={() => setShowModal(true)}>
                fetch subjects first
              </button>
            </p>
          )}

          {subjectsError && !showModal && (
            <div className="mt-3 px-3 py-2.5 rounded-lg bg-destructive/10 border border-destructive/20 text-xs text-destructive flex items-start gap-2">
              <AlertCircle size={13} className="shrink-0 mt-0.5" />
              <span>{subjectsError}</span>
            </div>
          )}

          {scraping && progress && (
            <div className="mt-3">
              <div className="flex items-center justify-between text-[11px] text-muted-foreground mb-1.5">
                <span>
                  {progress.course
                    ? `${progress.course}${progress.phase && progress.phase !== "complete" ? " · " + progress.phase : ""}…`
                    : "Starting…"}
                </span>
                <span>{progress.done} / {progress.total}</span>
              </div>
              <div className="h-1.5 rounded-full bg-surface-raised overflow-hidden">
                <div
                  className="h-full bg-primary transition-all duration-300"
                  style={{ width: `${progress.total ? (progress.done / progress.total) * 100 : 0}%` }}
                />
              </div>
            </div>
          )}
        </div>

        {/* Pipeline steps */}
        <PipelineSteps steps={steps} />
      </div>

      {/* Subject picker modal */}
      <Dialog
        open={showModal}
        onClose={() => setShowModal(false)}
        title="Subjects"
        description={
          noSubjects
            ? "No subjects loaded yet"
            : `${current.length} current · ${past.length} past · ${selectedIds.size} selected for sync`
        }
        className="max-w-lg"
      >
        {noSubjects ? (
          <div className="py-6 text-center">
            <BookOpen size={32} className="text-muted-foreground/40 mx-auto mb-3" />
            <p className="text-sm text-foreground font-medium mb-1">No subjects loaded</p>
            <p className="text-xs text-muted-foreground">Fetch your Canvas subjects to get started.</p>
          </div>
        ) : (
          <div className="space-y-5 max-h-[420px] overflow-y-auto -mx-6 px-6">
            {current.length > 0 && (
              <div>
                <p className="text-[11px] font-semibold text-primary uppercase tracking-wider mb-2 flex items-center gap-1.5">
                  <span className="w-1.5 h-1.5 rounded-full bg-primary inline-block" />
                  Current — {current[0]?.term_name}
                </p>
                <div className="space-y-1">
                  {current.map((s) => (
                    <SubjectRow
                      key={s.id}
                      subject={s}
                      checked={selectedIds.has(s.id)}
                      onToggle={() => toggleSubject(s.id)}
                    />
                  ))}
                </div>
              </div>
            )}

            {past.length > 0 && (
              <div>
                <button
                  onClick={() => setPastExpanded((v) => !v)}
                  className="flex items-center gap-2 text-[11px] font-semibold text-muted-foreground uppercase tracking-wider mb-2 hover:text-foreground transition-colors"
                >
                  {pastExpanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
                  Past subjects ({past.length})
                </button>
                {pastExpanded && (
                  <div className="space-y-3">
                    {Object.entries(pastBySemester)
                      .sort(([a], [b]) => b.localeCompare(a))
                      .map(([term, courses]) => (
                        <div key={term}>
                          <p className="text-[11px] text-muted-foreground mb-1.5 pl-1">{term}</p>
                          <div className="space-y-1">
                            {courses.map((s) => (
                              <SubjectRow
                                key={s.id}
                                subject={s}
                                checked={selectedIds.has(s.id)}
                                onToggle={() => toggleSubject(s.id)}
                                dimmed
                              />
                            ))}
                          </div>
                        </div>
                      ))}
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {subjectsError && (
          <div className="mt-3 px-3 py-2.5 rounded-lg bg-destructive/10 border border-destructive/20 text-xs text-destructive">
            {subjectsError}
          </div>
        )}

        <DialogFooter className="mt-4">
          <Button
            variant="outline"
            size="sm"
            onClick={handleRefetchSubjects}
            disabled={loadingSubjects || authStatus !== "connected"}
            className="mr-auto gap-1.5"
          >
            {loadingSubjects ? (
              <><Loader2 size={13} className="animate-spin" /> Fetching…</>
            ) : (
              <><RefreshCw size={13} /> Refetch Subjects</>
            )}
          </Button>
          <Button variant="outline" size="sm" onClick={() => setShowModal(false)}>
            Close
          </Button>
        </DialogFooter>
      </Dialog>
    </div>
  );
}
