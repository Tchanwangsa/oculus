import { useEffect, useState, useCallback, useRef } from "react";
import {
  RefreshCw,
  AlertCircle,
  BookOpen,
  ChevronRight,
  ChevronDown,
  Loader2,
  XCircle,
  Bug,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogFooter } from "@/components/ui/dialog";
import {
  upsertSubjects,
  getSubjects,
  getLastCompletedSyncRun,
  addLog,
  startSyncRun,
  type CanvasCourseRaw,
  type Subject,
  type SyncRun,
} from "@/lib/db";
import { fmtDate } from "@/lib/format";
import { useAuth, type AuthStatus } from "@/hooks/useAuth";
import { useSyncStore } from "@/stores/syncStore";
import { AuthCard } from "@/components/sync/AuthCard";
import { SubjectRow } from "@/components/subjects/SubjectRow";

const AUTH_BADGE: Record<
  AuthStatus,
  { label: string; variant: "success" | "secondary" | "warning" }
> = {
  connected: { label: "Connected", variant: "success" },
  disconnected: { label: "Disconnected", variant: "secondary" },
  pending: { label: "Signing in…", variant: "warning" },
};

const PHASE_LABEL: Record<string, string> = {
  home: "overview",
  announcements: "announcements",
  modules: "modules",
};

export default function SyncPage() {
  const { status: authStatus, disconnect: disconnectCanvas } = useAuth();
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [showModal, setShowModal] = useState(false);
  const [loadingSubjects, setLoadingSubjects] = useState(false);
  const [subjectsError, setSubjectsError] = useState<string | null>(null);
  const [lastSyncRun, setLastSyncRun] = useState<SyncRun | null>(null);
  const [pastExpanded, setPastExpanded] = useState(false);

  // Sync progress lives in the global store (survives navigation).
  const scraping = useSyncStore((s) => s.scraping);
  const progress = useSyncStore((s) => s.progress);
  const completedAt = useSyncStore((s) => s.completedAt);
  const syncError = useSyncStore((s) => s.error);
  const scrapingRef = useRef(false);

  useEffect(() => {
    scrapingRef.current = scraping;
  }, [scraping]);

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

  // Refresh the subject list when a sync run finishes.
  useEffect(() => {
    if (completedAt === 0) return;
    if (syncError) {
      setSubjectsError(`Sync error: ${syncError}`);
    } else {
      loadFromDb();
    }
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

    setSubjectsError(null);

    try {
      const runId = await startSyncRun();
      useSyncStore.getState().begin(runId, sel.length);
      await invoke("scrape_content", { subjects: sel });
    } catch (err) {
      useSyncStore.getState().fail(String(err));
      setSubjectsError(String(err));
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

  const phase = progress?.phase ? (PHASE_LABEL[progress.phase] ?? progress.phase) : null;

  return (
    <div className="flex flex-col h-full overflow-y-auto">
      {/* Header */}
      <div className="px-6 h-12 flex items-center gap-2.5 border-b border-border-subtle shrink-0">
        <span className="font-semibold text-[13px] text-foreground">Sync</span>
        <Badge variant={authBadge.variant}>{authBadge.label}</Badge>
        <Button
          variant="ghost"
          size="icon-sm"
          title="Open Canvas WebView DevTools"
          onClick={() => invoke("open_canvas_devtools")}
          className="ml-auto text-muted-foreground/60"
        >
          <Bug size={13} />
        </Button>
      </div>

      <div className="w-full max-w-lg mx-auto px-6 py-8 space-y-4">
        {/* Auth card */}
        <AuthCard
          status={authStatus}
          onConnect={async () => {
            try {
              await invoke("launch_canvas_auth");
            } catch {
              /* useAuth handles */
            }
          }}
          onDisconnect={async () => {
            setSubjects([]);
            setSelectedIds(new Set());
            await disconnectCanvas();
          }}
          scraping={scraping}
        />

        {/* Sync card */}
        <div className="rounded-lg border border-border bg-card p-4">
          <div className="flex items-center justify-between mb-0.5">
            <span className="font-medium text-[13px] text-foreground">
              Content
            </span>
            <span className="text-xs text-muted-foreground">
              Last synced {fmtDate(lastSyncRun?.finished_at ?? null)}
            </span>
          </div>
          <p className="text-xs text-muted-foreground mb-4">
            {selectedIds.size > 0
              ? `${selectedIds.size} subject${selectedIds.size === 1 ? "" : "s"} selected`
              : "No subjects selected"}
            {" · "}
            <button
              className="hover:text-foreground underline underline-offset-2 transition-colors"
              onClick={() => setShowModal(true)}
            >
              manage
            </button>
          </p>

          <div className="flex gap-2">
            <Button
              size="sm"
              className="flex-1 gap-2 h-8"
              onClick={handleSyncClick}
              disabled={authStatus !== "connected" || scraping}
            >
              {scraping ? (
                <>
                  <Loader2 size={13} className="animate-spin" /> Syncing…
                </>
              ) : (
                "Sync now"
              )}
            </Button>

            {scraping && (
              <Button
                variant="outline"
                size="sm"
                title="Cancel sync"
                onClick={handleCancel}
                className="shrink-0 h-8 gap-1.5 text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
              >
                <XCircle size={13} /> Cancel
              </Button>
            )}
          </div>

          {noSubjects && authStatus === "connected" && (
            <p className="text-xs text-muted-foreground mt-3 text-center">
              No subjects loaded —{" "}
              <button
                className="text-primary hover:underline"
                onClick={() => setShowModal(true)}
              >
                fetch subjects first
              </button>
            </p>
          )}

          {subjectsError && !showModal && (
            <div className="mt-3 px-3 py-2.5 rounded-md bg-destructive/10 border border-destructive/20 text-xs text-destructive flex items-start gap-2">
              <AlertCircle size={13} className="shrink-0 mt-0.5" />
              <span>{subjectsError}</span>
            </div>
          )}

          {scraping && progress && (
            <div className="mt-4">
              <div className="flex items-center justify-between text-xs text-muted-foreground mb-1.5">
                <span className="truncate">
                  {progress.course
                    ? `${progress.course}${phase && progress.phase !== "complete" ? ` — ${phase}` : ""}`
                    : "Starting…"}
                </span>
                <span className="tabular-nums shrink-0 ml-3">
                  {progress.done}/{progress.total}
                </span>
              </div>
              <div className="h-1 rounded-full bg-surface-raised overflow-hidden">
                <div
                  className="h-full bg-primary rounded-full transition-all duration-300"
                  style={{
                    width: `${progress.total ? (progress.done / progress.total) * 100 : 0}%`,
                  }}
                />
              </div>
            </div>
          )}
        </div>
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
            <BookOpen
              size={28}
              className="text-muted-foreground/40 mx-auto mb-3"
            />
            <p className="text-sm text-foreground font-medium mb-1">
              No subjects loaded
            </p>
            <p className="text-xs text-muted-foreground">
              Fetch your Canvas subjects to get started.
            </p>
          </div>
        ) : (
          <div className="space-y-5 max-h-[420px] overflow-y-auto -mx-6 px-6">
            {current.length > 0 && (
              <div>
                <p className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">
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
                  {pastExpanded ? (
                    <ChevronDown size={11} />
                  ) : (
                    <ChevronRight size={11} />
                  )}
                  Past subjects ({past.length})
                </button>
                {pastExpanded && (
                  <div className="space-y-3">
                    {Object.entries(pastBySemester)
                      .sort(([a], [b]) => b.localeCompare(a))
                      .map(([term, courses]) => (
                        <div key={term}>
                          <p className="text-[11px] text-muted-foreground mb-1.5 pl-1">
                            {term}
                          </p>
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
          <div className="mt-3 px-3 py-2.5 rounded-md bg-destructive/10 border border-destructive/20 text-xs text-destructive">
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
              <>
                <Loader2 size={13} className="animate-spin" /> Fetching…
              </>
            ) : (
              <>
                <RefreshCw size={13} /> Refetch Subjects
              </>
            )}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowModal(false)}
          >
            Close
          </Button>
        </DialogFooter>
      </Dialog>
    </div>
  );
}
