import { useEffect, useMemo, useRef, useState } from "react";
import { defaultSelection, type Provider } from "@/lib/harness";
import { ModelPicker } from "@/components/harness/ModelPicker";
import { useProviderModels } from "@/hooks/useProviderModels";
import {
  DEFAULT_JOB_MODELS,
  getJobModels,
  JOBS,
  setJobModels,
  type JobId,
  type JobModels,
  type JobSelection,
} from "@/lib/db";
import { Section } from "./section";

/**
 * Agent, model and reasoning level for each non-chat job, in the composer's
 * `ModelPicker` (no hidden "default"). Rust reads this straight from
 * `settings` (`harness::jobs`), so a change is live on the next run.
 */
function JobModelsSection() {
  const [jobs, setJobs] = useState<JobModels | null>(null);
  /** What is in the database, so the save effect can tell an edit from the load. */
  const saved = useRef<string | null>(null);

  useEffect(() => {
    getJobModels()
      .catch(() => structuredClone(DEFAULT_JOB_MODELS))
      .then((m) => {
        saved.current = JSON.stringify(m);
        setJobs(m);
      });
  }, []);

  // Only the agents a job is set to are asked for their catalogue.
  const needed = useMemo(() => (jobs ? JOBS.map((j) => jobs[j.id].provider) : []), [jobs]);
  const { providers, modelsFor } = useProviderModels(needed);

  const edit = (id: JobId, patch: Partial<JobSelection>) =>
    setJobs((prev) => (prev ? { ...prev, [id]: { ...prev[id], ...patch } } : prev));

  /** Save as you go, through one writer so a model change and the level
   *  change right after it can't land out of order. A row with no model yet
   *  (its list still arriving) waits. */
  useEffect(() => {
    if (!jobs || saved.current === null) return;
    if (JOBS.some((j) => !jobs[j.id].model)) return;
    const body = JSON.stringify(jobs);
    if (body === saved.current) return;
    saved.current = body;
    void setJobModels(jobs);
  }, [jobs]);

  /** Fill a model-less row once its provider's list arrives. */
  useEffect(() => {
    if (!jobs) return;
    for (const job of JOBS) {
      const row = jobs[job.id];
      if (row.model) continue;
      const pick = defaultSelection(modelsFor(row.provider));
      if (pick.model) edit(job.id, { model: pick.model, reasoningEffort: pick.reasoning });
    }
  }, [jobs, modelsFor]);

  /** Switching agent resets model and level: a Claude id means nothing to Codex. */
  const switchProvider = (id: JobId, provider: Provider) => {
    const pick = defaultSelection(modelsFor(provider));
    edit(id, { provider, model: pick.model ?? "", reasoningEffort: pick.reasoning });
  };

  return (
    <Section
      title="Jobs"
      description="Work the app hands to an agent on its own — no conversation, no timeline. Each job names the agent, the model and the reasoning level it runs on; a CLI flag overrides that for one run."
    >
      <div className="divide-y divide-border-subtle">
        {JOBS.map((job) => {
          const row = jobs?.[job.id];
          return (
            <div key={job.id} className="flex items-start justify-between gap-4 py-2.5">
              <div className="min-w-0">
                <div className="text-[13px] text-foreground">{job.label}</div>
                <div className="mt-0.5 text-xs text-muted-foreground">{job.description}</div>
              </div>
              {row && (
                <ModelPicker
                  providers={providers}
                  provider={row.provider}
                  providerLocked={false}
                  model={row.model || null}
                  reasoning={row.reasoningEffort}
                  onProvider={(p) => switchProvider(job.id, p)}
                  onModel={(m) => edit(job.id, { model: m ?? "" })}
                  onReasoning={(level) => edit(job.id, { reasoningEffort: level })}
                  className="-mr-1.5 mt-px shrink-0"
                />
              )}
            </div>
          );
        })}
      </div>
    </Section>
  );
}

export default function SettingsJobsPage() {
  return <JobModelsSection />;
}
