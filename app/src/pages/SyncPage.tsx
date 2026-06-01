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
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

type StepStatus = "done" | "pending" | "error" | "idle";

interface SyncStep {
  id: string;
  label: string;
  description: string;
  icon: typeof Globe;
  status: StepStatus;
}

const SYNC_STEPS: SyncStep[] = [
  {
    id: "auth",
    label: "Canvas Authentication",
    description: "SAML SSO via UniMelb identity provider",
    icon: ShieldCheck,
    status: "done",
  },
  {
    id: "courses",
    label: "Discover Subjects",
    description: "Enumerate enrolled courses from Canvas dashboard",
    icon: BookOpen,
    status: "done",
  },
  {
    id: "scrape",
    label: "Scrape Content",
    description: "Modules, lecture slides, assignments, notices",
    icon: Globe,
    status: "pending",
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
  done:    { icon: CheckCircle2, color: "text-success",     bg: "bg-success/10" },
  pending: { icon: RefreshCw,    color: "text-primary",     bg: "bg-primary/10" },
  error:   { icon: AlertCircle,  color: "text-destructive", bg: "bg-destructive/10" },
  idle:    { icon: Circle,       color: "text-muted-foreground", bg: "bg-surface-raised" },
};

const RECENT_ACTIVITY = [
  { time: "Today 09:14",  text: "Synced COMP30023 — 3 new pages",       status: "done"    },
  { time: "Today 09:12",  text: "Synced SWEN30006 — 1 new announcement", status: "done"    },
  { time: "Yesterday",    text: "Scrape failed: COMP30027 timeout",       status: "error"   },
  { time: "3 days ago",   text: "Full sync completed — 47 pages updated", status: "done"    },
];

export default function SyncPage() {
  return (
    <div className="flex flex-col h-full overflow-y-auto">
      <div className="px-6 h-14 flex items-center gap-3 border-b border-border shrink-0">
        <RefreshCw size={16} className="text-primary" />
        <span className="font-semibold text-foreground">Sync</span>
        <Badge variant="success">Connected</Badge>
      </div>

      <div className="px-6 py-6 space-y-6 max-w-2xl">
        {/* Auth status card */}
        <div className="rounded-xl border border-border bg-card p-5">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              <ShieldCheck size={16} className="text-success" />
              <span className="font-semibold text-sm text-foreground">Canvas Connection</span>
            </div>
            <Badge variant="success">Active</Badge>
          </div>
          <div className="flex items-center gap-3 text-xs text-muted-foreground">
            <Globe size={13} />
            <span>canvas.lms.unimelb.edu.au</span>
            <span className="ml-auto flex items-center gap-1">
              <Clock size={11} /> Last synced 2h ago
            </span>
          </div>
          <Separator className="my-4" />
          <div className="flex gap-2">
            <Button variant="outline" size="sm" className="flex-1">
              Re-authenticate
            </Button>
            <Button variant="ghost" size="sm" className="text-destructive hover:text-destructive hover:bg-destructive/10">
              Disconnect
            </Button>
          </div>
        </div>

        {/* Sync controls */}
        <div className="rounded-xl border border-border bg-card p-5">
          <p className="font-semibold text-sm text-foreground mb-4">Sync Controls</p>
          <div className="flex gap-2">
            <Button className="flex-1 gap-2">
              <RefreshCw size={14} />
              Sync All Subjects
            </Button>
            <Button variant="secondary" size="default">
              Choose Subjects
            </Button>
          </div>
          <p className="text-xs text-muted-foreground mt-3">
            Sync downloads modules, lecture slides, assignments, and notices into your local knowledge base.
          </p>
        </div>

        {/* Pipeline steps */}
        <div className="rounded-xl border border-border bg-card p-5">
          <p className="font-semibold text-sm text-foreground mb-4">Sync Pipeline</p>
          <div className="space-y-1">
            {SYNC_STEPS.map((step, i) => {
              const config = STATUS_CONFIG[step.status];
              const Icon = config.icon;
              const StepIcon = step.icon;

              return (
                <div key={step.id}>
                  <div className="flex items-start gap-4 py-3">
                    <div className={cn("w-8 h-8 rounded-lg flex items-center justify-center shrink-0 mt-0.5", config.bg)}>
                      <StepIcon size={14} className={config.color} />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-foreground">{step.label}</span>
                        <Icon
                          size={13}
                          className={cn(config.color, step.status === "pending" && "animate-spin")}
                        />
                      </div>
                      <p className="text-xs text-muted-foreground mt-0.5">{step.description}</p>
                    </div>
                    <ChevronRight size={14} className="text-muted-foreground shrink-0 mt-1" />
                  </div>
                  {i < SYNC_STEPS.length - 1 && <Separator />}
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
                <div className="w-1.5 h-1.5 rounded-full mt-2 shrink-0"
                  style={{ backgroundColor: item.status === "done" ? "var(--color-success)" : "var(--color-destructive)" }}
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
