import { useCallback, useMemo, useState } from "react";

import { useProviderModels } from "@/hooks/agents/useProviderModels";
import { JOBS, type JobId } from "@/lib/db";
import type { Provider } from "@/lib/harness";
import { Section } from "@/components/settings/shared/section";
import { JobModelsDialog } from "./JobModelsDialog";
import { JobRow } from "./JobRow";
import { useJobChains } from "./useJobChains";

/**
 * Each non-chat job's chain of models — agent, model and reasoning level for
 * each, in the composer's `ModelPicker` (no hidden "default"). Rust reads the
 * chains straight from `settings` (`harness::jobs`), so a change is live on
 * the next run.
 */
export function JobsSection() {
  const { jobs, editChain } = useJobChains();
  const [open, setOpen] = useState<JobId | null>(null);

  // Catalogues are asked only of agents a chain names or a picker tab has
  // shown, so an agent nobody uses never has its CLI spawned.
  const [viewed, setViewed] = useState<ReadonlySet<Provider>>(new Set());
  const onView = useCallback(
    (p: Provider) => setViewed((s) => (s.has(p) ? s : new Set(s).add(p))),
    [],
  );
  const needed = useMemo(
    () => [
      ...new Set([...viewed, ...(jobs ? JOBS.flatMap((j) => jobs[j.id].map((s) => s.provider)) : [])]),
    ],
    [jobs, viewed],
  );
  const { providers } = useProviderModels(needed);

  const openJob = open ? JOBS.find((j) => j.id === open) : undefined;

  return (
    <Section
      title="Jobs"
      description="Work the app hands to an agent on its own — no conversation, no timeline. Each job names the agent, the model and the reasoning level it runs on, with fallbacks for a failed run; a CLI flag overrides that for one run."
    >
      <div className="divide-y divide-border-subtle">
        {jobs &&
          JOBS.map((job) => (
            <JobRow
              key={job.id}
              job={job.id}
              label={job.label}
              description={job.description}
              chain={jobs[job.id]}
              providers={providers}
              onChange={(edit) => editChain(job.id, edit)}
              onView={onView}
              onFallbacks={() => setOpen(job.id)}
            />
          ))}
      </div>
      {jobs && openJob && (
        <JobModelsDialog
          job={openJob.id}
          label={openJob.label}
          chain={jobs[openJob.id]}
          providers={providers}
          onChange={(edit) => editChain(openJob.id, edit)}
          onView={onView}
          onClose={() => setOpen(null)}
        />
      )}
    </Section>
  );
}
