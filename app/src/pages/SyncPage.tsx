import { useEffect, useState, useCallback, useRef } from "react";
import {
  RefreshCw, CheckCircle2, Circle, AlertCircle, Globe,
  ShieldCheck, BookOpen, Database, ChevronRight, ChevronDown,
  Clock, Loader2, XCircle, Settings, Bug,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { Dialog, DialogFooter } from "@/components/ui/dialog";
import {
  upsertSubjects, getSubjects, getLastCompletedSyncRun, addLog,
  startSyncRun, finishSyncRun, markSubjectSynced, upsertFile,
  type CanvasCourseRaw, type Subject, type SyncRun,
} from "@/lib/db";
import { cn } from "@/lib/utils";

// ── Types ─────────────────────────────────────────────────────────────────────

type AuthStatus = "connected" | "disconnected" | "pending";
type StepStatus = "done" | "pending" | "error" | "idle";

interface CanvasCourse {
  id: number;
  name: string;
  course_code: string;
  workflow_state: "available" | "completed" | "unpublished";
  start_at: string | null;
  end_at: string | null;
  term?: { id: number; name: string };
  _oculus_is_current: boolean;
}

interface SyncStep { id: string; label: string; description: string; icon: typeof Globe; status: StepStatus }

// ── Static config ─────────────────────────────────────────────────────────────

const PIPELINE_STEPS: SyncStep[] = [
  { id: "auth",    label: "Canvas Authentication", description: "SAML SSO via UniMelb identity provider",           icon: ShieldCheck, status: "idle" },
  { id: "courses", label: "Load Subjects",          description: "Fetch enrolled courses from Canvas API",           icon: BookOpen,    status: "idle" },
  { id: "scrape",  label: "Sync Content",           description: "Modules, lecture slides, assignments, notices",    icon: Globe,       status: "idle" },
  { id: "graph",   label: "Build Knowledge Graph",  description: "Ingest into FalkorDB — nodes, edges, metadata",   icon: Database,    status: "idle" },
];

const STATUS_CFG: Record<StepStatus, { icon: typeof Circle; cls: string; bg: string }> = {
  done:    { icon: CheckCircle2, cls: "text-success",          bg: "bg-success/10"     },
  pending: { icon: Loader2,      cls: "text-primary",          bg: "bg-primary/10"     },
  error:   { icon: AlertCircle,  cls: "text-destructive",      bg: "bg-destructive/10" },
  idle:    { icon: Circle,       cls: "text-muted-foreground", bg: "bg-surface-raised" },
};

const AUTH_BADGE: Record<AuthStatus, { label: string; variant: "success" | "secondary" | "warning" }> = {
  connected:    { label: "Connected",    variant: "success"   },
  disconnected: { label: "Disconnected", variant: "secondary" },
  pending:      { label: "Signing in…",  variant: "warning"   },
};

function fmtDate(iso: string | null): string {
  if (!iso) return "Never";
  const d = new Date(iso);
  const now = new Date();
  const diff = now.getTime() - d.getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1)  return "Just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24)  return `${hrs}h ago`;
  return d.toLocaleDateString();
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function SyncPage() {
  const [authStatus, setAuthStatus]         = useState<AuthStatus>("disconnected");
  const [steps, setSteps]                   = useState(PIPELINE_STEPS);
  const [subjects, setSubjects]             = useState<Subject[]>([]);
  const [selectedIds, setSelectedIds]       = useState<Set<number>>(new Set());
  const [showModal, setShowModal]           = useState(false);
  const [loadingSubjects, setLoadingSubjects] = useState(false);
  const [subjectsError, setSubjectsError]   = useState<string | null>(null);
  const [lastSyncRun, setLastSyncRun]       = useState<SyncRun | null>(null);
  const [pastExpanded, setPastExpanded]     = useState(false);
  const [scraping, setScraping]             = useState(false);
  const [progress, setProgress]             = useState<{ done: number; total: number; course?: string; phase?: string } | null>(null);
  const runIdRef                            = useRef<number | null>(null);
  const scrapingRef                         = useRef(false);

  // Keep ref in sync so event listeners that close over [] deps can read current value.
  useEffect(() => { scrapingRef.current = scraping; }, [scraping]);

  // ── Boot ──────────────────────────────────────────────────────────────────

  const loadFromDb = useCallback(async () => {
    const [rows, lastRun] = await Promise.all([
      getSubjects(),
      getLastCompletedSyncRun(),
    ]);
    setSubjects(rows);
    setLastSyncRun(lastRun);
    // Default-select current subjects
    setSelectedIds((prev) => {
      const next = new Set(prev);
      rows.filter((s) => s.is_current).forEach((s) => next.add(s.id));
      return next;
    });
  }, []);

  useEffect(() => {
    invoke<boolean>("get_auth_status").then((ok) => {
      if (ok) {
        setAuthStatus("connected");
        setSteps((p) => p.map((s) => s.id === "auth" ? { ...s, status: "done" } : s));
      }
    });
    loadFromDb();
  }, [loadFromDb]);

  // ── Auth events ───────────────────────────────────────────────────────────

  useEffect(() => {
    const subs = [
      listen("canvas-auth-success", () => {
        setAuthStatus("connected");
        setSteps((p) => p.map((s) => s.id === "auth" ? { ...s, status: "done" } : s));
      }),
      listen("canvas-auth-cancelled", () => {
        setAuthStatus((p) => p === "pending" ? "disconnected" : p);
      }),
      listen("canvas-auth-expired", () => {
        setAuthStatus("disconnected");
        setSteps(PIPELINE_STEPS);
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
      listen<CanvasCourse[]>("subjects-loaded", async (e) => {
        setLoadingSubjects(false);
        setSubjectsError(null);
        setSteps((p) => p.map((s) => s.id === "courses" ? { ...s, status: "done" } : s));
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
        setSteps((p) => p.map((s) => s.id === "courses" ? { ...s, status: "error" } : s));
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
          } catch (err) {
            console.error("upsertFile failed:", err);
          }
        }
      ),
      listen<{ done: number; total: number; course?: string; phase?: string }>("scrape-progress", (e) => {
        setProgress(e.payload);
      }),
      listen<{ level: string; course: string; message: string }>("scrape-log", (e) => {
        const { level, message } = e.payload;
        addLog(message, level === "error" ? "error" : level === "warning" ? "warning" : "info").catch(() => {});
      }),
      listen<{ count: number; cancelled?: boolean }>("scrape-complete", async (e) => {
        const runId = runIdRef.current;
        if (runId != null) {
          await finishSyncRun(runId, "completed", e.payload.count, e.payload.count);
          await addLog(`Synced ${e.payload.count} subject homepage(s)`);
        }
        setScraping(false);
        setProgress(null);
        setSteps((p) => p.map((s) => s.id === "scrape" ? { ...s, status: "done" } : s));
        await loadFromDb();
      }),
      listen<string>("scrape-error", async (e) => {
        const runId = runIdRef.current;
        if (runId != null) await finishSyncRun(runId, "failed", 0, 0, e.payload);
        setScraping(false);
        setProgress(null);
        setSubjectsError(`Scrape error: ${e.payload}`);
        setSteps((p) => p.map((s) => s.id === "scrape" ? { ...s, status: "error" } : s));
      }),
    ];
    return () => { subs.forEach((p) => p.then((f) => f())); };
  }, [loadFromDb]);

  // ── Handlers ──────────────────────────────────────────────────────────────

  const handleAuth = async () => {
    setAuthStatus("pending");
    try { await invoke("launch_canvas_auth"); }
    catch { setAuthStatus("disconnected"); }
  };

  const handleDisconnect = async () => {
    setAuthStatus("disconnected");
    setSteps(PIPELINE_STEPS);
    setSubjects([]);
    setSelectedIds(new Set());
    try { await invoke("disconnect_canvas"); } catch { /* ignore */ }
  };

  const handleRefetchSubjects = async () => {
    setLoadingSubjects(true);
    setSubjectsError(null);
    setSteps((p) => p.map((s) => s.id === "courses" ? { ...s, status: "pending" } : s));
    try { await invoke("sync_subjects"); }
    catch (err) {
      setLoadingSubjects(false);
      setSubjectsError(String(err));
      setSteps((p) => p.map((s) => s.id === "courses" ? { ...s, status: "error" } : s));
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
    setSteps((p) => p.map((s) => s.id === "scrape" ? { ...s, status: "pending" } : s));

    try {
      runIdRef.current = await startSyncRun();
      await invoke("scrape_content", { subjects: sel });
    } catch (err) {
      setScraping(false);
      setProgress(null);
      setSubjectsError(String(err));
      setSteps((p) => p.map((s) => s.id === "scrape" ? { ...s, status: "error" } : s));
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

  const authBadge    = AUTH_BADGE[authStatus];
  const current      = subjects.filter((s) => s.is_current);
  const past         = subjects.filter((s) => !s.is_current);
  const noSubjects   = subjects.length === 0;

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

        {/* ── Canvas Connection card ────────────────────────────────────── */}
        <div className="rounded-xl border border-border bg-card p-5">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              {authStatus === "connected" ? (
                <ShieldCheck size={16} className="text-success" />
              ) : authStatus === "pending" ? (
                <Loader2 size={16} className="text-primary animate-spin" />
              ) : (
                <XCircle size={16} className="text-muted-foreground" />
              )}
              <span className="font-semibold text-sm text-foreground">Canvas Connection</span>
            </div>
            <Badge variant={authBadge.variant}>{authBadge.label}</Badge>
          </div>

          <div className="flex items-center gap-3 text-xs text-muted-foreground mb-4">
            <Globe size={13} />
            <span>canvas.lms.unimelb.edu.au</span>
            {authStatus === "connected" && (
              <span className="ml-auto flex items-center gap-1">
                <Clock size={11} /> Session active
              </span>
            )}
          </div>

          <Separator className="mb-4" />

          <div className="flex gap-2">
            <Button
              variant={authStatus === "connected" ? "outline" : "default"}
              size="sm"
              className="flex-1"
              onClick={handleAuth}
              disabled={authStatus === "pending"}
            >
              {authStatus === "pending" ? (
                <><Loader2 size={13} className="animate-spin" /> Opening Canvas login…</>
              ) : authStatus === "connected" ? (
                "Re-authenticate"
              ) : (
                "Connect to Canvas"
              )}
            </Button>
            {authStatus === "connected" && (
              <Button
                variant="ghost"
                size="sm"
                className="text-destructive hover:text-destructive hover:bg-destructive/10"
                onClick={handleDisconnect}
                disabled={scraping}
                title={scraping ? "Cannot disconnect while syncing — cancel first" : undefined}
              >
                Disconnect
              </Button>
            )}
          </div>

          {authStatus === "pending" && (
            <p className="text-xs text-muted-foreground mt-3 text-center">
              Complete sign-in in the Canvas window, then return here.
            </p>
          )}
        </div>

        {/* ── Sync card ─────────────────────────────────────────────────── */}
        <div className="rounded-xl border border-border bg-card p-5">
          {/* Card header */}
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

          <Separator className="mb-4" />

          {/* Action row */}
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
              <button
                className="text-primary hover:underline"
                onClick={() => setShowModal(true)}
              >
                fetch subjects first
              </button>
            </p>
          )}

          {/* Inline error (auth expiry, pre-flight failure, etc.) */}
          {subjectsError && !showModal && (
            <div className="mt-3 px-3 py-2.5 rounded-lg bg-destructive/10 border border-destructive/20 text-xs text-destructive flex items-start gap-2">
              <AlertCircle size={13} className="shrink-0 mt-0.5" />
              <span>{subjectsError}</span>
            </div>
          )}

          {/* Progress */}
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

        {/* ── Pipeline card ─────────────────────────────────────────────── */}
        <div className="rounded-xl border border-border bg-card p-5">
          <p className="font-semibold text-sm text-foreground mb-4">Sync Pipeline</p>
          <div className="space-y-1">
            {steps.map((step, i) => {
              const cfg = STATUS_CFG[step.status];
              const StatusIcon = cfg.icon;
              const StepIcon = step.icon;
              return (
                <div key={step.id}>
                  <div className="flex items-start gap-4 py-3">
                    <div className={cn("w-8 h-8 rounded-lg flex items-center justify-center shrink-0 mt-0.5", cfg.bg)}>
                      <StepIcon size={14} className={cfg.cls.split(" ")[0]} />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-foreground">{step.label}</span>
                        <StatusIcon size={13} className={cn(cfg.cls, step.status === "pending" && "animate-spin")} />
                      </div>
                      <p className="text-xs text-muted-foreground mt-0.5">{step.description}</p>
                    </div>
                    <ChevronRight size={14} className="text-muted-foreground shrink-0 mt-1" />
                  </div>
                  {i < steps.length - 1 && <Separator />}
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* ── Subject picker modal ──────────────────────────────────────────── */}
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
            <p className="text-xs text-muted-foreground">
              Fetch your Canvas subjects to get started.
            </p>
          </div>
        ) : (
          <div className="space-y-5 max-h-[420px] overflow-y-auto -mx-6 px-6">
            {/* Current */}
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

            {/* Past — collapsible */}
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

// ── Sub-components ────────────────────────────────────────────────────────────

function SubjectRow({
  subject, checked, onToggle, dimmed = false,
}: {
  subject: Subject;
  checked: boolean;
  onToggle: () => void;
  dimmed?: boolean;
}) {
  return (
    <button
      onClick={onToggle}
      className={cn(
        "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg border text-left transition-colors",
        checked
          ? "bg-primary/5 border-primary/20"
          : "bg-surface border-border hover:border-border/80",
        dimmed && !checked && "opacity-60"
      )}
    >
      <div className={cn(
        "w-4 h-4 rounded border-2 flex items-center justify-center shrink-0 transition-colors",
        checked ? "bg-primary border-primary" : "border-muted-foreground/40"
      )}>
        {checked && (
          <svg width="8" height="6" viewBox="0 0 8 6" fill="none">
            <path d="M1 3L3 5L7 1" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        )}
      </div>
      <BookOpen size={13} className={checked ? "text-primary shrink-0" : "text-muted-foreground shrink-0"} />
      <div className="min-w-0 flex-1">
        <p className="text-xs font-medium text-foreground truncate">{subject.name}</p>
        <p className="text-[11px] text-muted-foreground">{subject.code}</p>
      </div>
    </button>
  );
}
