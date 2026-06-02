import {
  CheckCircle2,
  Circle,
  AlertCircle,
  Loader2,
  ShieldCheck,
  BookOpen,
  Globe,
  Database,
  ChevronRight,
} from "lucide-react";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

type StepStatus = "done" | "pending" | "error" | "idle";

const STATUS_CFG: Record<StepStatus, { icon: typeof Circle; cls: string; bg: string }> = {
  done:    { icon: CheckCircle2, cls: "text-success",          bg: "bg-success/10"     },
  pending: { icon: Loader2,      cls: "text-primary",          bg: "bg-primary/10"     },
  error:   { icon: AlertCircle,  cls: "text-destructive",      bg: "bg-destructive/10" },
  idle:    { icon: Circle,       cls: "text-muted-foreground", bg: "bg-surface-raised" },
};

const STEPS = [
  { id: "auth",    label: "Canvas Authentication", description: "SAML SSO via UniMelb identity provider",           icon: ShieldCheck },
  { id: "courses", label: "Load Subjects",          description: "Fetch enrolled courses from Canvas API",           icon: BookOpen    },
  { id: "scrape",  label: "Sync Content",           description: "Modules, lecture slides, assignments, notices",    icon: Globe       },
  { id: "graph",   label: "Build Knowledge Graph",  description: "Ingest into FalkorDB — nodes, edges, metadata",   icon: Database    },
];

interface PipelineStepsProps {
  steps: { id: string; status: StepStatus }[];
}

export function PipelineSteps({ steps }: PipelineStepsProps) {
  const stepMap = new Map(steps.map((s) => [s.id, s.status]));

  return (
    <div className="rounded-xl border border-border bg-card p-5">
      <p className="font-semibold text-sm text-foreground mb-4">Sync Pipeline</p>
      <div className="space-y-1">
        {STEPS.map((step, i) => {
          const status = stepMap.get(step.id) ?? "idle";
          const cfg = STATUS_CFG[status];
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
                    <StatusIcon size={13} className={cn(cfg.cls, status === "pending" && "animate-spin")} />
                  </div>
                  <p className="text-xs text-muted-foreground mt-0.5">{step.description}</p>
                </div>
                <ChevronRight size={14} className="text-muted-foreground shrink-0 mt-1" />
              </div>
              {i < STEPS.length - 1 && <Separator />}
            </div>
          );
        })}
      </div>
    </div>
  );
}
