import { Button } from "@/components/ui/button";
import { DEFAULT_JOB_MODELS, type JobChain, type JobId } from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";
import { useJobAttention } from "@/stores/agents/jobAttentionStore";
import { ChainPicker } from "./ChainPicker";
import { JobAttention } from "./JobAttention";
import { withEntry } from "./chain";

/** One job: what it does, what the audit found, and its first model, with
 *  the fallbacks one click away in `JobModelsDialog`. */
export function JobRow({
  job,
  label,
  description,
  chain,
  providers,
  onChange,
  onView,
  onFallbacks,
}: {
  job: JobId;
  label: string;
  description: string;
  chain: JobChain;
  providers: PickerProvider[];
  onChange: (edit: (chain: JobChain) => JobChain) => void;
  onView: (provider: Provider) => void;
  onFallbacks: () => void;
}) {
  const off = chain.length === 0;
  // A removal notice already says the job is off.
  const noticed = useJobAttention((s) => s.notices.some((n) => n.job === job));
  const fallbacks = chain.length - 1;

  return (
    <div className="flex items-start justify-between gap-4 py-2.5">
      <div className="min-w-0">
        <div className="text-[13px] text-foreground">{label}</div>
        <div className="mt-0.5 text-xs text-muted-foreground">{description}</div>
        {off && !noticed && (
          <div className="mt-1 text-xs text-muted-foreground">
            Off — choose a model to run this job.
          </div>
        )}
        <JobAttention job={job} off={off} providers={providers} />
      </div>
      <div className="-mr-1.5 mt-px flex shrink-0 flex-col items-end gap-0.5">
        <ChainPicker
          providers={providers}
          entry={chain[0] ?? null}
          fallbackProvider={DEFAULT_JOB_MODELS[job][0].provider}
          onPick={(pick) => onChange((c) => withEntry(c, 0, pick))}
          onView={onView}
          align="end"
          emptyLabel="None"
        />
        <Button
          variant="ghost"
          size="xs"
          onClick={onFallbacks}
          className="px-1.5 font-normal text-muted-foreground hover:text-foreground"
        >
          {fallbacks < 1 ? "Add fallbacks" : `${fallbacks} fallback${fallbacks === 1 ? "" : "s"}`}
        </Button>
      </div>
    </div>
  );
}
