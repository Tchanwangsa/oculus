import { useEffect, useState } from "react";
import { Sparkle } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useProviderModels } from "@/hooks/useProviderModels";
import { getJobModels, type JobSelection } from "@/lib/db";
import { reasoningLabel } from "@/lib/harness";
import { cn } from "@/lib/utils";

import type { SuggestStatus } from "./editor/aiSuggest";

/** Keeps the note focused, so turning suggestions on mid-sentence works on
 *  the next keystroke. */
const keepFocus = (e: React.MouseEvent) => e.preventDefault();

/**
 * The topbar's AI-suggestions switch. A pulse while a request is out; a red
 * dot, with the message in the tooltip, from a failure until the next
 * request succeeds.
 */
export function SuggestToggle({
  on,
  onChange,
  status,
}: {
  on: boolean;
  onChange: (on: boolean) => void;
  status: SuggestStatus;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-pressed={on}
          aria-label="AI suggestions"
          onMouseDown={keepFocus}
          onClick={() => onChange(!on)}
          className={cn(
            "relative",
            on ? "text-brand hover:text-brand" : "text-muted-foreground hover:text-foreground",
          )}
        >
          <Sparkle
            size={14}
            weight={on ? "fill" : "regular"}
            className={cn(on && status.pending && "animate-pulse")}
          />
          {on && status.error && (
            <span
              aria-hidden
              className="absolute top-0.5 right-0.5 size-1.5 rounded-full bg-destructive"
            />
          )}
        </Button>
      </TooltipTrigger>
      <TooltipContent className="flex flex-col items-start gap-0.5">
        <SuggestTooltip error={on ? status.error : null} />
      </TooltipContent>
    </Tooltip>
  );
}

/** Mounted only while the tooltip shows, so the job and its model's name are
 *  read on hover, current with Settings, and never on page load. */
function SuggestTooltip({ error }: { error: string | null }) {
  const [job, setJob] = useState<JobSelection | null>(null);
  useEffect(() => {
    let live = true;
    getJobModels()
      .then((m) => live && setJob(m.documentSuggestions))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, []);
  const { modelsFor } = useProviderModels(job ? job.provider : []);
  const model = job ? (modelsFor(job.provider).find((m) => m.id === job.model)?.label ?? job.model) : null;
  const line = ["AI suggestions", model, job?.reasoningEffort ? reasoningLabel(job.reasoningEffort) : null]
    .filter(Boolean)
    .join(" · ");
  return (
    <>
      {line}
      <span className="text-[11px] text-background/60">Change in Settings → AI</span>
      {error && <span className="text-[11px] text-destructive">{error}</span>}
    </>
  );
}
