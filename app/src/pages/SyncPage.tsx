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
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

type AuthStatus = "connected" | "disconnected" | "pending";
type StepStatus = "done" | "pending" | "error" | "idle";

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
    label: "Discover Subjects",
    description: "Enumerate enrolled courses from Canvas dashboard",
    icon: BookOpen,
    status: "idle",
  },
  {
    id: "scrape",
    label: "Scrape Content",
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

const STATUS_CONFIG: Record<StepStatus, { icon: typeof Circle; color: string; bg: string }> = {
  done:    { icon: CheckCircle2, color: "text-success",           bg: "bg-success/10" },
  pending: { icon: Loader2,      color: "text-primary animate-spin", bg: "bg-primary/10" },
  error:   { icon: AlertCircle,  color: "text-destructive",       bg: "bg-destructive/10" },
  idle:    { icon: Circle,       color: "text-muted-foreground",  bg: "bg-surface-raised" },
};

const AUTH_BADGE: Record<AuthStatus, { label: string; variant: "success" | "secondary" | "warning" }> = {
  connected:    { label: "Connected",    variant: "success" },
  disconnected: { label: "Disconnected", variant: "secondary" },
  pending:      { label: "Signing in…",  variant: "warning" },
};

const RECENT_ACTIVITY = [
  { time: "Today 09:14",  text: "Synced COMP30023 — 3 new pages",        ok: true  },
  { time: "Today 09:12",  text: "Synced SWEN30006 — 1 new announcement", ok: true  },
  { time: "Yesterday",    text: "Scrape failed: COMP30027 timeout",       ok: false },
  { time: "3 days ago",   text: "Full sync completed — 47 pages updated", ok: true  },
];

export default function SyncPage() {
  const [authStatus, setAuthStatus] = useState<AuthStatus>("disconnected");
  const [steps, setSteps] = useState(PIPELINE_STEPS);

  // Check initial auth state from Rust
  useEffect(() => {
    invoke<boolean>("get_auth_status").then((authenticated) => {
      if (authenticated) setAuthStatus("connected");
    });
  }, []);

  // Listen for auth events from Rust
  useEffect(() => {
    const successUnsub = listen("canvas-auth-success", () => {
      setAuthStatus("connected");
      setSteps((prev) =>
        prev.map((s) => (s.id === "auth" ? { ...s, status: "done" } : s))
      );
    });

    const cancelUnsub = listen("canvas-auth-cancelled", () => {
      setAuthStatus((prev) => (prev === "pending" ? "disconnected" : prev));
    });

    return () => {
      successUnsub.then((f) => f());
      cancelUnsub.then((f) => f());
    };
  }, []);

  const handleAuth = async () => {
    setAuthStatus("pending");
    try {
      await invoke("launch_canvas_auth");
    } catch (err) {
      console.error("Auth launch failed:", err);
      setAuthStatus("disconnected");
    }
  };

  const handleDisconnect = () => {
    setAuthStatus("disconnected");
    setSteps(PIPELINE_STEPS);
  };

  const authBadge = AUTH_BADGE[authStatus];

  return (
    <div className="flex flex-col h-full overflow-y-auto">
      {/* Header */}
      <div className="px-6 h-14 flex items-center gap-3 border-b border-border shrink-0">
        <RefreshCw size={16} className="text-primary" />
        <span className="font-semibold text-foreground">Sync</span>
        <Badge variant={authBadge.variant}>{authBadge.label}</Badge>
      </div>

      <div className="px-6 py-6 space-y-5 max-w-2xl">
        {/* Auth status card */}
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
                <>
                  <Loader2 size={13} className="animate-spin" />
                  Opening Canvas login…
                </>
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

        {/* Sync controls — only shown when connected */}
        {authStatus === "connected" && (
          <div className="rounded-xl border border-border bg-card p-5">
            <p className="font-semibold text-sm text-foreground mb-4">Sync Controls</p>
            <div className="flex gap-2">
              <Button className="flex-1 gap-2">
                <RefreshCw size={14} />
                Sync All Subjects
              </Button>
              <Button variant="secondary">Choose Subjects</Button>
            </div>
            <p className="text-xs text-muted-foreground mt-3">
              Downloads modules, lecture slides, assignments, and notices into your local knowledge base.
            </p>
          </div>
        )}

        {/* Pipeline steps */}
        <div className="rounded-xl border border-border bg-card p-5">
          <p className="font-semibold text-sm text-foreground mb-4">Sync Pipeline</p>
          <div className="space-y-1">
            {steps.map((step, i) => {
              const cfg = STATUS_CONFIG[step.status];
              const StatusIcon = cfg.icon;
              const StepIcon = step.icon;

              return (
                <div key={step.id}>
                  <div className="flex items-start gap-4 py-3">
                    <div className={cn("w-8 h-8 rounded-lg flex items-center justify-center shrink-0 mt-0.5", cfg.bg)}>
                      <StepIcon size={14} className={cfg.color.split(" ")[0]} />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-foreground">{step.label}</span>
                        <StatusIcon size={13} className={cfg.color} />
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

        {/* Activity log */}
        <div className="rounded-xl border border-border bg-card p-5">
          <p className="font-semibold text-sm text-foreground mb-4">Recent Activity</p>
          <div className="space-y-3">
            {RECENT_ACTIVITY.map((item, i) => (
              <div key={i} className="flex items-start gap-3">
                <div
                  className="w-1.5 h-1.5 rounded-full mt-2 shrink-0"
                  style={{
                    backgroundColor: item.ok
                      ? "var(--color-success)"
                      : "var(--color-destructive)",
                  }}
                />
                <div className="flex-1 min-w-0">
                  <p className="text-xs text-foreground">{item.text}</p>
                  <p className="text-[11px] text-muted-foreground mt-0.5">{item.time}</p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
