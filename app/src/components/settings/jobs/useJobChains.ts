import { useCallback, useEffect, useRef, useState } from "react";

import { useWindowEvent } from "@/hooks/backend/useEvents";
import {
  cleanJobModels,
  DEFAULT_JOB_MODELS,
  dismissJobNotices,
  getJobModels,
  JOB_MODELS_CHANGED_EVENT,
  setJobModels,
  type JobChain,
  type JobId,
  type JobModels,
} from "@/lib/db";

/**
 * Every job's chain as stored, saved as it is edited. It reloads whenever the
 * chains are written elsewhere (the audit removing a model that left its
 * agent's list), so the page never saves back a chain that still holds it.
 * `saved` is what the database holds: a load sets it, so the save effect
 * doesn't echo what was just read, and the reload after the page's own write
 * finds nothing new.
 */
export function useJobChains() {
  const [jobs, setJobs] = useState<JobModels | null>(null);
  const saved = useRef<string | null>(null);
  // A load that started before an edit was saved would put the old chain back
  // on screen; the save's own change event reloads after it instead.
  const writes = useRef(0);
  const loads = useRef(0);

  const load = useCallback(() => {
    const seq = ++loads.current;
    const writesAtStart = writes.current;
    getJobModels()
      .catch(() => structuredClone(DEFAULT_JOB_MODELS))
      .then((m) => {
        if (seq !== loads.current || writesAtStart !== writes.current) return;
        const body = JSON.stringify(cleanJobModels(m));
        if (body === saved.current) return;
        saved.current = body;
        setJobs(m);
      });
  }, []);

  useEffect(load, [load]);
  useWindowEvent(JOB_MODELS_CHANGED_EVENT, load);

  // One writer, so a model pick and the level change after it can't land out
  // of order.
  useEffect(() => {
    if (!jobs || saved.current === null) return;
    const body = JSON.stringify(cleanJobModels(jobs));
    if (body === saved.current) return;
    saved.current = body;
    writes.current += 1;
    void setJobModels(jobs);
  }, [jobs]);

  /** Applies `edit` to the job's current chain; any notice of a model removed
   *  from it is answered by the edit, so it goes too. */
  const editChain = useCallback((job: JobId, edit: (chain: JobChain) => JobChain) => {
    setJobs((prev) => (prev ? { ...prev, [job]: edit(prev[job]) } : prev));
    void dismissJobNotices(job);
  }, []);

  return { jobs, editChain };
}
