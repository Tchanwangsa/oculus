import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useWindowEvent } from "@/hooks/backend/useEvents";
import { useProviderModels } from "@/hooks/agents/useProviderModels";
import {
  addJobNotices,
  getJobModels,
  getJobNotices,
  JOB_MODELS_CHANGED_EVENT,
  JOBS,
  setJobModels,
  type JobModels,
  type JobNotice,
  type JobSelection,
} from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";
import { providerOf } from "@/lib/harness/opencodeCatalogue";
import { useJobAttention, type FlaggedJobModel } from "@/stores/agents/jobAttentionStore";

/** Catalogues are asked for this long after launch, so the audit adds no CLI
 *  or server spawn to app start. */
const AUDIT_DELAY_MS = 10_000;

/** Claude lists some models only with a context tag (`claude-opus-5-5[1m]`)
 *  while the bare id still runs, so the tag is ignored when matching. */
const bareId = (id: string) => id.replace(/\[[^\]]*\]$/, "");

function listed(models: PickerProvider["models"], id: string): boolean {
  const bare = bareId(id);
  return models.some((m) => m.id === id || bareId(m.id) === bare);
}

/** Gone only on a definite answer: the CLI is there, its list arrived and is
 *  not empty, and the model isn't on it. A failed fetch, a signed-out
 *  provider or a missing CLI never removes anything. */
function isGone(entry: JobSelection, providers: PickerProvider[]): boolean {
  const p = providers.find((x) => x.id === entry.provider);
  if (!p || p.health === "missing" || p.loading || p.models.length === 0) return false;
  // opencode's list spans its signed-in providers; one signed out (or hidden)
  // since is unreachable, not gone, so only a provider still listing models counts.
  if (entry.provider === "opencode") {
    const sub = providerOf(entry.model);
    if (!p.models.some((m) => providerOf(m.id) === sub)) return false;
  }
  return !listed(p.models, entry.model);
}

/** The model's display name from any list that still has it, else its id. */
function labelFor(model: string, providers: PickerProvider[]): string {
  for (const p of providers) {
    const hit = p.models.find((m) => m.id === model);
    if (hit) return hit.label;
  }
  return model;
}

/** Drops gone entries in place — later fallbacks move up; nothing is ever
 *  substituted, so a job whose every model left is off, not defaulted. */
function prune(models: JobModels, providers: PickerProvider[]) {
  const next = structuredClone(models);
  const removed: JobNotice[] = [];
  const at = new Date().toISOString();
  for (const job of JOBS) {
    next[job.id] = models[job.id].filter((entry) => {
      if (!isGone(entry, providers)) return true;
      removed.push({
        job: job.id,
        provider: entry.provider,
        model: entry.model,
        label: labelFor(entry.model, providers),
        at,
      });
      return false;
    });
  }
  return { next, removed };
}

/**
 * Keeps every job's chain to models its agent still offers, and publishes
 * what Settings should badge through `useJobAttention`. Mounted once, in
 * `AppLayout`. A model gone from its agent's list is removed and recorded as
 * a `JobNotice`; one whose CLI is missing is only flagged, since the CLI may
 * just be off PATH.
 */
export function useJobAudit() {
  const [models, setModels] = useState<JobModels | null>(null);
  const [notices, setNotices] = useState<JobNotice[]>([]);
  const [armed, setArmed] = useState(false);
  const loadSeq = useRef(0);

  const load = useCallback(() => {
    const seq = ++loadSeq.current;
    Promise.all([getJobModels(), getJobNotices()])
      .then(([m, n]) => {
        if (seq !== loadSeq.current) return;
        setModels(m);
        setNotices(n);
      })
      .catch(() => {});
  }, []);

  useEffect(load, [load]);
  useWindowEvent(JOB_MODELS_CHANGED_EVENT, load);

  useEffect(() => {
    const timer = window.setTimeout(() => setArmed(true), AUDIT_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, []);

  const needed = useMemo<Provider[]>(
    () =>
      armed && models ? [...new Set(JOBS.flatMap((j) => models[j.id].map((s) => s.provider)))] : [],
    [armed, models],
  );
  // Never starts opencode: only Settings → opencode may (docs/harness.md).
  const { providers } = useProviderModels(needed, { start: false });

  // One prune at a time; a trigger that lands mid-run re-runs it after.
  const running = useRef(false);
  const again = useRef(false);
  const [rerun, setRerun] = useState(0);

  useEffect(() => {
    if (!armed || !models) return;
    if (!JOBS.some((j) => models[j.id].some((s) => isGone(s, providers)))) return;
    if (running.current) {
      again.current = true;
      return;
    }
    running.current = true;
    void (async () => {
      // Re-read: a save from Settings since the last load must not be undone.
      const { next, removed } = prune(await getJobModels(), providers);
      if (removed.length === 0) return;
      // Notices first: a notice whose removal failed is re-made by the next
      // audit, while a removal without its notice would be silent.
      await addJobNotices(removed);
      await setJobModels(next);
    })()
      .catch(() => {})
      .finally(() => {
        running.current = false;
        if (again.current) {
          again.current = false;
          setRerun((n) => n + 1);
        }
      });
  }, [armed, models, providers, rerun]);

  const flagged = useMemo<FlaggedJobModel[]>(() => {
    if (!models) return [];
    const missing = new Set(providers.filter((p) => p.health === "missing").map((p) => p.id));
    return JOBS.flatMap((j) =>
      models[j.id]
        .filter((s) => missing.has(s.provider))
        .map((s) => ({ job: j.id, provider: s.provider, model: s.model })),
    );
  }, [models, providers]);

  useEffect(() => {
    useJobAttention.setState({
      notices,
      flagged,
      attention: notices.length > 0 || flagged.length > 0,
    });
  }, [notices, flagged]);
}
