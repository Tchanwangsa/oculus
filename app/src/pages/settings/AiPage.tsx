import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ArrowsClockwise, CircleNotch } from "@phosphor-icons/react";
import {
  defaultSelection,
  harnessHealth,
  type BridgeHealth,
  type Provider,
} from "@/lib/harness";
import { ModelPicker } from "@/components/harness/ModelPicker";
import { useProviderModels } from "@/hooks/useProviderModels";
import { Button } from "@/components/ui/button";
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
 * The CLI agents behind Chat: where each binary was found and which version,
 * or why it was not. Rechecking clears the cached lookup, for right after an
 * install.
 */
function CliAgentsSection() {
  const [health, setHealth] = useState<BridgeHealth[] | null>(null);
  const [checking, setChecking] = useState(false);
  const check = useCallback(() => {
    setChecking(true);
    harnessHealth()
      .then(setHealth)
      .catch(() => setHealth([]))
      .finally(() => setChecking(false));
  }, []);
  useEffect(check, [check]);

  return (
    <Section
      title="CLI agents"
      description="Chat runs Claude Code, Codex or opencode from your own machine, signed in as you — no API key, no per-token billing."
    >
      <div className="divide-y divide-border-subtle">
        {(health ?? []).map((h) => (
          <div key={h.provider} className="flex items-start justify-between gap-4 py-2.5">
            <div className="min-w-0">
              <div className="text-[13px] text-foreground">{h.label}</div>
              <div className="mt-0.5 truncate text-xs text-muted-foreground">
                {h.path ?? (
                  <>
                    Not found. Install it, or set <span className="text-foreground">{h.overrideEnv}</span> to the binary.
                  </>
                )}
              </div>
              {h.error && h.path && <div className="mt-0.5 text-xs text-destructive">{h.error}</div>}
            </div>
            <div className="shrink-0 text-xs tabular-nums text-muted-foreground">
              {h.version ? `v${h.version}` : h.path ? "—" : "missing"}
            </div>
          </div>
        ))}
        {health === null && (
          <div className="py-2.5 text-xs text-muted-foreground">Checking…</div>
        )}
      </div>
      <Button variant="ghost" size="xs" className="mt-2" onClick={check} disabled={checking}>
        {checking ? <CircleNotch size={12} className="animate-spin" /> : <ArrowsClockwise size={12} />}
        Recheck
      </Button>
    </Section>
  );
}

/**
 * Which agent runs each model-backed job that is not a chat turn, on what
 * model, at what reasoning level.
 *
 * The same `ModelPicker` the composer uses, for the same reason: what the row
 * shows is what the CLI is told, and no "default" hides behind it. The jobs
 * themselves run in Rust and read this straight back out of `settings`
 * (`harness::jobs`), so a change here is live on the next run — there is no
 * second copy of the selection anywhere.
 */
function JobModelsSection() {
  const [jobs, setJobs] = useState<JobModels | null>(null);
  /** What is already in the database, so the save effect below can tell an
   *  edit from the load that started it. */
  const saved = useRef<string | null>(null);

  useEffect(() => {
    getJobModels()
      .catch(() => structuredClone(DEFAULT_JOB_MODELS))
      .then((m) => {
        saved.current = JSON.stringify(m);
        setJobs(m);
      });
  }, []);

  // An agent whose catalogue comes from its CLI is asked only when a job is
  // actually set to it — a row has to show its model's name rather than its
  // id — and not merely because this page was opened. Switching a row is what
  // asks, since `needed` changes with it.
  const needed = useMemo(() => (jobs ? JOBS.map((j) => jobs[j.id].provider) : []), [jobs]);
  const { providers, modelsFor } = useProviderModels(needed);

  const edit = (id: JobId, patch: Partial<JobSelection>) =>
    setJobs((prev) => (prev ? { ...prev, [id]: { ...prev[id], ...patch } } : prev));

  /** Settings save as you go, like the rest of this page. One writer rather
   *  than one per control: a model change and the level change that follows it
   *  are two edits a beat apart, and two overlapping writes of the whole
   *  object can land in either order. A row with no model yet — Codex picked,
   *  its list still arriving — is a moment, not a configuration, so it waits. */
  useEffect(() => {
    if (!jobs || saved.current === null) return;
    if (JOBS.some((j) => !jobs[j.id].model)) return;
    const body = JSON.stringify(jobs);
    if (body === saved.current) return;
    saved.current = body;
    void setJobModels(jobs);
  }, [jobs]);

  /** A row sits without a model only while a provider's list is still coming
   *  — every catalogue but Claude's is fetched — so it is filled the moment
   *  one exists, exactly as the composer fills an empty selection. */
  useEffect(() => {
    if (!jobs) return;
    for (const job of JOBS) {
      const row = jobs[job.id];
      if (row.model) continue;
      const pick = defaultSelection(modelsFor(row.provider));
      if (pick.model) edit(job.id, { model: pick.model, reasoningEffort: pick.reasoning });
    }
  }, [jobs, modelsFor]);

  /** Switching agent takes the model and the level with it: a Claude model id
   *  means nothing to Codex. Claude's list is compiled in so the new selection
   *  is immediate; a fetched one arrives with its list, above. */
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

export default function SettingsAiPage() {
  return (
    <div className="flex flex-col gap-8">
      <CliAgentsSection />
      <JobModelsSection />
    </div>
  );
}
