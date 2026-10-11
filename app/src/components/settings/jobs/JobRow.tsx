import { Stack, StackPlus } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { DEFAULT_JOB_MODELS, type JobChain, type JobId } from "@/lib/db";
import { reasoningLabel, type PickerProvider, type Provider } from "@/lib/harness";
import { useJobAttention } from "@/stores/agents/jobAttentionStore";
import { ChainPicker } from "./ChainPicker";
import { JobAttention } from "./JobAttention";
import { modelLabel, withEntry } from "./chain";

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
  const fallbacks = chain.slice(1);
  const fallbacksTip =
    fallbacks.length === 0
      ? "Add fallbacks"
      : `Then ${fallbacks
          .map((f) =>
            [
              modelLabel(providers, f.provider, f.model),
              f.reasoningEffort && reasoningLabel(f.reasoningEffort),
            ]
              .filter(Boolean)
              .join(" "),
          )
          .join(", then ")}`;

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
      <div className="-mr-1.5 flex shrink-0 items-center gap-0.5">
        <ChainPicker
          providers={providers}
          entry={chain[0] ?? null}
          fallbackProvider={DEFAULT_JOB_MODELS[job][0].provider}
          onPick={(pick) => onChange((c) => withEntry(c, 0, pick))}
          onView={onView}
          align="end"
          emptyLabel="None"
        />
        <Tooltip>
          <TooltipTrigger asChild>
            {/* Always icon-sized, the count a badge on the icon, so every
                row's model picker lines up. */}
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={fallbacks.length > 0 ? `${fallbacks.length} fallbacks` : "Add fallbacks"}
              onClick={onFallbacks}
              className="relative text-muted-foreground hover:text-foreground"
            >
              {fallbacks.length > 0 ? (
                <>
                  <Stack className="size-3.5" />
                  <span className="absolute right-0 top-0 flex h-3 min-w-3 items-center justify-center rounded-full bg-primary px-[3px] text-[9px] font-medium leading-none tabular-nums text-primary-foreground">
                    {fallbacks.length}
                  </span>
                </>
              ) : (
                <StackPlus className="size-3.5" />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{fallbacksTip}</TooltipContent>
        </Tooltip>
      </div>
    </div>
  );
}
