import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ArrowsClockwise, CircleNotch } from "@phosphor-icons/react";
import {
  defaultSelection,
  harnessAntigravityRevoke,
  harnessAntigravityRules,
  splitRule,
  type Provider,
} from "@/lib/harness";
import { ModelPicker } from "@/components/harness/ModelPicker";
import { useBridgeHealth } from "@/hooks/useBridgeHealth";
import { useProviderModels } from "@/hooks/useProviderModels";
import { signInAccount, signInState, useSignInStatus } from "@/hooks/useSignInStatus";
import { SignInDialog, useSignIn } from "@/components/harness/SignInDialog";
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
 *
 * A row whose CLI *is* here answers the second question too — whether it is
 * signed in, and as whom. Being installed and being usable are different
 * facts, and until this stage the second one only surfaced as a red row in the
 * timeline after a turn had already failed. Sign-in status is its own shared
 * read (`app/src/hooks/useSignInStatus.ts`) rather than a field on health,
 * because Rust answers it by spawning the CLI and asking, where health is a
 * cached lookup — so *Recheck* drops both caches and asks both questions.
 *
 * opencode's row says nothing here, and that is the contract rather than an
 * omission: its credentials are per provider, so `harness_sign_in_status`
 * answers `signedIn: null` and the providers strip below is the answer.
 */
function CliAgentsSection() {
  const { health, recheck, checking } = useBridgeHealth();
  const { statuses, recheck: recheckSignIn, checking: checkingSignIn } = useSignInStatus();
  /** One button, both questions — a row that said "installed" and went on
   *  saying "signed out" a minute after signing in would be the row that
   *  lies. */
  const recheckAll = useCallback(() => {
    recheck();
    recheckSignIn();
  }, [recheck, recheckSignIn]);

  const install = useAgentInstall(recheck);
  const signIn = useSignIn(recheckSignIn);
  /** Which row's dialog is open. Separate from the run above: the run
   *  outlives the dialog, and the dialog can be opened on a row with no run. */
  const [openFor, setOpenFor] = useState<{ provider: Provider; label: string } | null>(null);
  const [signInFor, setSignInFor] = useState<Provider | null>(null);

  return (
    <Section
      title="CLI agents"
      description="Chat runs Claude Code, Codex, opencode or Antigravity from your own machine, signed in as you — no API key, no per-token billing."
    >
      <div className="divide-y divide-border-subtle">
        {(health ?? []).map((h) => {
          // `unknown` — the probe has not landed, or this is opencode — draws
          // nothing at all, the same three-state discipline the model picker
          // uses for health.
          const state = h.path ? signInState(statuses, h.provider) : "unknown";
          const account = signInAccount(statuses, h.provider);
          return (
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
                {state !== "unknown" && (
                  <div className="mt-0.5 text-xs text-muted-foreground">
                    {state === "in" ? (account ? `Signed in — ${account}` : "Signed in") : "Signed out"}
                  </div>
                )}
                {h.error && h.path && <div className="mt-0.5 text-xs text-destructive">{h.error}</div>}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <span className="text-xs tabular-nums text-muted-foreground">
                  {h.version ? `v${h.version}` : h.path ? "—" : "missing"}
                </span>
                {state === "out" && (
                  <Button variant="outline" size="xs" onClick={() => setSignInFor(h.provider)}>
                    {signIn.run?.provider === h.provider && !signIn.run.result ? (
                      <>
                        <CircleNotch size={12} className="animate-spin" />
                        Signing in…
                      </>
                    ) : (
                      "Sign in"
                    )}
                  </Button>
                )}
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
          );
        })}
        {health === null && (
          <div className="py-2.5 text-xs text-muted-foreground">Checking…</div>
        )}
      </div>
      <Button
        variant="ghost"
        size="xs"
        className="mt-2"
        onClick={recheckAll}
        disabled={checking || checkingSignIn}
      >
        {checking || checkingSignIn ? (
          <CircleNotch size={12} className="animate-spin" />
        ) : (
          <ArrowsClockwise size={12} />
        )}
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
      {signInFor && (
        <SignInDialog
          provider={signInFor}
          run={signIn.run?.provider === signInFor ? signIn.run : null}
          onStart={() => signIn.start(signInFor)}
          onCode={(code) => signIn.submitCode(code)}
          onCancel={() => signIn.cancel()}
          onClose={() => {
            // Same rule as the install above: a finished run is cleared with
            // the dialog, one still in flight is kept and resumes on reopen.
            if (signIn.run?.result) signIn.clear();
            setSignInFor(null);
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
   *  — every catalogue is fetched from its CLI — so it is filled the moment
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
   *  means nothing to Codex. A list already fetched makes the new selection
   *  immediate; one still coming arrives with its list, above. */
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

/** An approval in the words the chat's allow button used, not `agy`'s
 *  syntax. */
function ruleWords(rule: string): string {
  const r = splitRule(rule);
  if (!r) return rule;
  switch (r.action) {
    case "command":
      return `Run ${r.value}`;
    case "write_file":
      return `Write in ${r.value}`;
    case "read_file":
      return `Read ${r.value}`;
    case "read_url":
      return `Read pages on ${r.value}`;
  }
}

/**
 * What the student has let Antigravity do, from the allow button on a
 * refusal in chat (`app/src/components/harness/PermissionCard.tsx`), and the
 * way to take one back.
 *
 * Absent until there is something to list: most students never run
 * Antigravity, and a section that only ever said "none" would be furniture.
 * Once it has shown, it stays for the visit, so removing the last approval
 * does not pull the page up under the pointer. Loading it is a settings-row
 * read in Rust; nothing is spawned.
 */
function AntigravityApprovalsSection() {
  const [rules, setRules] = useState<string[] | null>(null);
  const [shown, setShown] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    harnessAntigravityRules()
      .then((r) => {
        setRules(r);
        if (r.length) setShown(true);
      })
      .catch(() => setRules([]));
  }, []);

  const revoke = async (rule: string) => {
    setRemoving(rule);
    setError(null);
    try {
      setRules(await harnessAntigravityRevoke(rule));
    } catch (e) {
      setError(String(e));
    } finally {
      setRemoving(null);
    }
  };

  if (!shown || !rules) return null;
  return (
    <Section
      title="Antigravity approvals"
      description="What you let Antigravity do when it stopped to ask. They are kept in agy's own global settings, so agy in your terminal follows them too."
    >
      <div className="divide-y divide-border-subtle">
        {rules.map((rule) => (
          <div key={rule} className="flex items-center justify-between gap-4 py-2.5">
            <div className="min-w-0 truncate text-[13px] text-foreground" title={rule}>
              {ruleWords(rule)}
            </div>
            <Button
              variant="outline"
              size="xs"
              className="shrink-0"
              disabled={removing !== null}
              onClick={() => void revoke(rule)}
            >
              {removing === rule && <CircleNotch size={12} className="animate-spin" />}
              Remove
            </Button>
          </div>
        ))}
        {rules.length === 0 && (
          <div className="py-2.5 text-xs text-muted-foreground">No approvals.</div>
        )}
      </div>
      {error && <p className="mt-2 break-words text-xs text-destructive">{error}</p>}
    </Section>
  );
}

export default function SettingsAiPage() {
  return (
    <div className="flex flex-col gap-8">
      <CliAgentsSection />
      <OpencodeProvidersSection />
      <JobModelsSection />
      <AntigravityApprovalsSection />
    </div>
  );
}
