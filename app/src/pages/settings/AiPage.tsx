import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowsClockwise, CircleNotch } from "@phosphor-icons/react";
import { defaultSelection, type Provider } from "@/lib/harness";
import { ModelPicker } from "@/components/harness/ModelPicker";
import { useBridgeHealth } from "@/hooks/useBridgeHealth";
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
import { OpencodeProvidersSection } from "@/components/settings/OpencodeProvidersSection";
import {
  InstallAgentDialog,
  useAgentInstall,
} from "@/components/settings/InstallAgentDialog";
import { Section } from "./section";

/**
 * The CLI agents behind Chat: where each binary was found and which version,
 * or why it was not.
 *
 * This is no longer the only reader — every model picker asks the same
 * question now, and a provider with no binary reads as *not installed* there
 * rather than offering a catalogue nothing can run — so the answer comes from
 * the shared `useBridgeHealth`. This page keeps the one thing that is its
 * alone: *Recheck* is the only caller that passes `recheck`, which is what
 * drops Rust's cached lookups and pays for a fresh probe after an install.
 *
 * It is also the only place that can *do* something about a missing one. A
 * row with no path carries *Install*, which opens the commands this machine
 * can run and runs the one that is picked
 * (`app/src/components/settings/InstallAgentDialog.tsx`). The run is held
 * here rather than in that dialog so closing it mid-install strands neither
 * the output nor the recheck the finish fires — and `recheck` is exactly the
 * hand-off, since a CLI installed a second ago stays missing until Rust's
 * cached failure is dropped.
 */
function CliAgentsSection() {
  const { health, recheck, checking } = useBridgeHealth();
  const install = useAgentInstall(recheck);
  /** Which row's dialog is open. Separate from the run above: the run
   *  outlives the dialog, and the dialog can be opened on a row with no run. */
  const [openFor, setOpenFor] = useState<{ provider: Provider; label: string } | null>(null);

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
            <div className="flex shrink-0 items-center gap-2">
              <span className="text-xs tabular-nums text-muted-foreground">
                {h.version ? `v${h.version}` : h.path ? "—" : "missing"}
              </span>
              {!h.path && (
                <Button
                  variant="outline"
                  size="xs"
                  onClick={() => setOpenFor({ provider: h.provider, label: h.label })}
                >
                  {install.run?.provider === h.provider && !install.run.result ? (
                    <>
                      <CircleNotch size={12} className="animate-spin" />
                      Installing…
                    </>
                  ) : (
                    "Install"
                  )}
                </Button>
              )}
            </div>
          </div>
        ))}
        {health === null && (
          <div className="py-2.5 text-xs text-muted-foreground">Checking…</div>
        )}
      </div>
      <Button variant="ghost" size="xs" className="mt-2" onClick={recheck} disabled={checking}>
        {checking ? <CircleNotch size={12} className="animate-spin" /> : <ArrowsClockwise size={12} />}
        Recheck
      </Button>
      {openFor && (
        <InstallAgentDialog
          provider={openFor.provider}
          label={openFor.label}
          run={install.run?.provider === openFor.provider ? install.run : null}
          onStart={(route) => install.start(openFor.provider, route)}
          onClose={() => {
            // A finished run is cleared with the dialog, so reopening the row
            // offers the commands again rather than a log of what already
            // happened. One still running is kept, and reopening resumes it.
            if (install.run?.result) install.clear();
            setOpenFor(null);
          }}
        />
      )}
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
      <OpencodeProvidersSection />
      <JobModelsSection />
    </div>
  );
}
