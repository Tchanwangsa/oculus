import { useEffect, useState } from "react";
import {
  RefreshCw,
  CheckCircle2,
  Circle,
  AlertCircle,
  Globe,
  ShieldCheck,
  BookOpen,
  Database,
  ChevronRight,
  Clock,
  Loader2,
  XCircle,
  Download,
  Bug,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { Dialog, DialogFooter } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

type AuthStatus = "connected" | "disconnected" | "pending";
type StepStatus = "done" | "pending" | "error" | "idle";

interface CanvasCourse {
  id: number;
  name: string;
  course_code: string;
  workflow_state: "available" | "completed" | "unpublished";
  start_at: string | null;
  end_at: string | null;
  term?: { id: number; name: string; start_at?: string; end_at?: string };
  _oculus_is_current: boolean;
}

interface SyncStep {
  id: string;
  label: string;
  description: string;
  icon: typeof Globe;
  status: StepStatus;
}

const PIPELINE_STEPS: SyncStep[] = [
  {
    id: "auth",
    label: "Canvas Authentication",
    description: "SAML SSO via UniMelb identity provider",
    icon: ShieldCheck,
    status: "idle",
  },
  {
    id: "courses",
    label: "Load Subjects",
    description: "Fetch enrolled courses from Canvas API",
    icon: BookOpen,
    status: "idle",
  },
  {
    id: "scrape",
    label: "Sync Content",
    description: "Modules, lecture slides, assignments, notices",
    icon: Globe,
    status: "idle",
  },
  {
    id: "graph",
    label: "Build Knowledge Graph",
    description: "Ingest into FalkorDB — nodes, edges, metadata",
    icon: Database,
    status: "idle",
  },
];

const STATUS_CFG: Record<StepStatus, { icon: typeof Circle; cls: string; bg: string }> = {
  done:    { icon: CheckCircle2, cls: "text-success",     bg: "bg-success/10" },
  pending: { icon: Loader2,      cls: "text-primary",     bg: "bg-primary/10" },
  error:   { icon: AlertCircle,  cls: "text-destructive", bg: "bg-destructive/10" },
  idle:    { icon: Circle,       cls: "text-muted-foreground", bg: "bg-surface-raised" },
};

const AUTH_BADGE: Record<AuthStatus, { label: string; variant: "success" | "secondary" | "warning" }> = {
  connected:    { label: "Connected",   variant: "success" },
  disconnected: { label: "Disconnected",variant: "secondary" },
  pending:      { label: "Signing in…", variant: "warning" },
};

const RECENT_ACTIVITY = [
  { time: "Today 09:14",  text: "Synced COMP30023 — 3 new pages",        ok: true  },
  { time: "Today 09:12",  text: "Synced SWEN30006 — 1 new announcement", ok: true  },
  { time: "Yesterday",    text: "Sync failed: COMP30027 timeout",         ok: false },
  { time: "3 days ago",   text: "Full sync completed — 47 pages updated", ok: true  },
];

export default function SyncPage() {
  const [authStatus, setAuthStatus]             = useState<AuthStatus>("disconnected");
  const [steps, setSteps]                       = useState(PIPELINE_STEPS);
  const [loadingSubjects, setLoadingSubjects]   = useState(false);
  const [subjectsError, setSubjectsError]       = useState<string | null>(null);
  const [fetchedCourses, setFetchedCourses]     = useState<CanvasCourse[]>([]);
  const [showSubjectsDialog, setShowSubjectsDialog] = useState(false);

  // Check initial auth state from Rust
  useEffect(() => {
    invoke<boolean>("get_auth_status").then((ok) => {
      if (ok) {
        setAuthStatus("connected");
        setSteps((prev) => prev.map((s) => s.id === "auth" ? { ...s, status: "done" } : s));
      }
    });
  }, []);

  // Auth events
  useEffect(() => {
    const successUnsub = listen("canvas-auth-success", () => {
      setAuthStatus("connected");
      setSteps((prev) => prev.map((s) => s.id === "auth" ? { ...s, status: "done" } : s));
    });
    const cancelUnsub = listen("canvas-auth-cancelled", () => {
      setAuthStatus((prev) => prev === "pending" ? "disconnected" : prev);
    });
    return () => {
      successUnsub.then((f) => f());
      cancelUnsub.then((f) => f());
    };
  }, []);

  // Subjects events
  useEffect(() => {
    const loadedUnsub = listen<CanvasCourse[]>("subjects-loaded", (e) => {
      setFetchedCourses(e.payload);
      setLoadingSubjects(false);
      setSteps((prev) => prev.map((s) => s.id === "courses" ? { ...s, status: "done" } : s));
      setShowSubjectsDialog(true);
    });
    const errorUnsub = listen<string>("subjects-error", (e) => {
      setSubjectsError(e.payload);
      setLoadingSubjects(false);
      setSteps((prev) => prev.map((s) => s.id === "courses" ? { ...s, status: "error" } : s));
    });
    return () => {
      loadedUnsub.then((f) => f());
      errorUnsub.then((f) => f());
    };
  }, []);

  const handleAuth = async () => {
    setAuthStatus("pending");
    try {
      await invoke("launch_canvas_auth");
    } catch {
      setAuthStatus("disconnected");
    }
  };

  const handleDisconnect = async () => {
    setAuthStatus("disconnected");
    setSteps(PIPELINE_STEPS);
    setFetchedCourses([]);
    try { await invoke("disconnect_canvas"); } catch { /* ignore */ }
  };

  const handleSyncSubjects = async () => {
    setLoadingSubjects(true);
    setSubjectsError(null);
    setSteps((prev) => prev.map((s) => s.id === "courses" ? { ...s, status: "pending" } : s));
    try {
      await invoke("sync_subjects");
    } catch (err) {
      setLoadingSubjects(false);
      setSubjectsError(String(err));
      setSteps((prev) => prev.map((s) => s.id === "courses" ? { ...s, status: "error" } : s));
    }
  };

  const authBadge = AUTH_BADGE[authStatus];

  const currentCourses = fetchedCourses.filter((c) => c._oculus_is_current);
  const pastCourses    = fetchedCourses.filter((c) => !c._oculus_is_current);

  // Group past courses by term name
  const pastBySemester = pastCourses.reduce<Record<string, CanvasCourse[]>>((acc, c) => {
    const key = c.term?.name ?? "Unknown term";
    (acc[key] ??= []).push(c);
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
                title="Resets local session — clears stored Canvas cookies"
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

        {/* Load Subjects — only when connected */}
        {authStatus === "connected" && (
          <div className="rounded-xl border border-border bg-card p-5">
            <div className="flex items-center justify-between mb-1">
              <p className="font-semibold text-sm text-foreground">Subjects</p>
              {fetchedCourses.length > 0 && (
                <Badge variant="default">{fetchedCourses.length} loaded</Badge>
              )}
            </div>
            <p className="text-xs text-muted-foreground mb-4">
              Fetches all current and past subjects from Canvas. Future subjects (page not live) are excluded.
            </p>

            {subjectsError && (
              <div className="mb-3 px-3 py-2.5 rounded-lg bg-destructive/10 border border-destructive/20 text-xs text-destructive">
                {subjectsError}
              </div>
            )}

            <div className="flex gap-2 mb-0">
              <Button
                variant="ghost"
                size="icon-sm"
                title="Open Canvas WebView DevTools (debug)"
                onClick={() => invoke("open_canvas_devtools")}
                className="text-muted-foreground"
              >
                <Bug size={13} />
              </Button>
            </div>

            <Button
              className="w-full gap-2 mt-2"
              onClick={handleSyncSubjects}
              disabled={loadingSubjects}
            >
              {loadingSubjects ? (
                <><Loader2 size={14} className="animate-spin" /> Fetching subjects…</>
              ) : fetchedCourses.length > 0 ? (
                <><RefreshCw size={14} /> Refresh Subjects</>
              ) : (
                <><Download size={14} /> Load Subjects</>
              )}
            </Button>

            {fetchedCourses.length > 0 && !loadingSubjects && (
              <button
                className="mt-2 w-full text-xs text-muted-foreground hover:text-foreground transition-colors py-1.5"
                onClick={() => setShowSubjectsDialog(true)}
              >
                View {fetchedCourses.length} loaded subjects →
              </button>
            )}
          </div>
        )}

        {/* Pipeline */}
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

        {/* Activity */}
        <div className="rounded-xl border border-border bg-card p-5">
          <p className="font-semibold text-sm text-foreground mb-4">Recent Activity</p>
          <div className="space-y-3">
            {RECENT_ACTIVITY.map((item, i) => (
              <div key={i} className="flex items-start gap-3">
                <div
                  className="w-1.5 h-1.5 rounded-full mt-2 shrink-0"
                  style={{ backgroundColor: item.ok ? "var(--color-success)" : "var(--color-destructive)" }}
                />
                <div>
                  <p className="text-xs text-foreground">{item.text}</p>
                  <p className="text-[11px] text-muted-foreground mt-0.5">{item.time}</p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Subjects Dialog */}
      <Dialog
        open={showSubjectsDialog}
        onClose={() => setShowSubjectsDialog(false)}
        title="Your Subjects"
        description={`${currentCourses.length} current · ${pastCourses.length} past · ${fetchedCourses.length} total`}
        className="max-w-lg"
      >
        <div className="space-y-5 max-h-[480px] overflow-y-auto -mx-6 px-6">

          {/* Current semester */}
          {currentCourses.length > 0 && (
            <div>
              <p className="text-[11px] font-semibold text-primary uppercase tracking-wider mb-2 flex items-center gap-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-primary inline-block" />
                Current — {currentCourses[0]?.term?.name}
              </p>
              <div className="space-y-1.5">
                {currentCourses.map((c) => (
                  <div
                    key={c.id}
                    className="flex items-center gap-3 px-3 py-2.5 rounded-lg bg-primary/5 border border-primary/15"
                  >
                    <BookOpen size={13} className="text-primary shrink-0" />
                    <div className="min-w-0">
                      <p className="text-xs font-medium text-foreground truncate">{c.name}</p>
                      <p className="text-[11px] text-muted-foreground">{c.course_code}</p>
                    </div>
                    <Badge variant="default" className="ml-auto shrink-0">Current</Badge>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Past semesters grouped by term */}
          {Object.entries(pastBySemester)
            .sort(([a], [b]) => b.localeCompare(a))
            .map(([term, courses]) => (
              <div key={term}>
                <p className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">
                  {term}
                </p>
                <div className="space-y-1.5">
                  {courses.map((c) => (
                    <div
                      key={c.id}
                      className="flex items-center gap-3 px-3 py-2.5 rounded-lg bg-surface border border-border"
                    >
                      <BookOpen size={13} className="text-muted-foreground shrink-0" />
                      <div className="min-w-0">
                        <p className="text-xs font-medium text-foreground truncate">{c.name}</p>
                        <p className="text-[11px] text-muted-foreground">{c.course_code}</p>
                      </div>
                      <Badge variant="secondary" className="ml-auto shrink-0">Past</Badge>
                    </div>
                  ))}
                </div>
              </div>
            ))}
        </div>
        <DialogFooter className="mt-4">
          <Button variant="outline" size="sm" onClick={() => setShowSubjectsDialog(false)}>
            Close
          </Button>
          <Button size="sm" disabled>
            Sync All →
          </Button>
        </DialogFooter>
      </Dialog>
    </div>
  );
}
