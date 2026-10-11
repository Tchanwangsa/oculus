import { getSetting, setSetting } from "./settings";
import { isProvider, type Provider } from "@/lib/harness/providers";

// Each non-chat model job names an ordered list of models: the first, then
// fallbacks tried when a run fails. One JSON value, read back in Rust by
// `harness::jobs`, where the jobs run. An empty list turns the job off.

/** Mirrors `Job` in `app/src-tauri/src/harness/jobs.rs`. */
export type JobId =
  | "lectureChapters"
  | "lectureEnd"
  | "threadNaming"
  | "documentSuggestions";

/** `reasoningEffort` is null only for a model that takes no level. */
export interface JobSelection {
  provider: Provider;
  model: string;
  reasoningEffort: string | null;
}

/** First choice, then fallbacks in order. `[]` is "none": the job is off. */
export type JobChain = JobSelection[];

export type JobModels = Record<JobId, JobChain>;

/** The jobs Settings → Jobs lists, in the order it lists them. */
export const JOBS: { id: JobId; label: string; description: string }[] = [
  {
    id: "lectureChapters",
    label: "Lecture chapters",
    description:
      "Reads a recording's slide frames and transcript and names its topics. One long turn, eight to eleven minutes.",
  },
  {
    id: "lectureEnd",
    label: "Lecture end",
    description:
      "Finds where a lecture's content ends, so Done and Up Next don't wait for the Q&A. One short turn per lecture.",
  },
  {
    id: "threadNaming",
    label: "Chat thread names",
    description:
      "One line naming a conversation from its first exchange, once, after the first reply.",
  },
  {
    id: "documentSuggestions",
    label: "Document suggestions",
    description:
      "Inline ghost-text completions in notes, half a second after you stop typing. One short turn per pause.",
  },
];

const JOB_IDS = new Set<string>(JOBS.map((j) => j.id));

function isJobId(value: unknown): value is JobId {
  return typeof value === "string" && JOB_IDS.has(value);
}

/** Mirrors `default_selection` in `app/src-tauri/src/harness/jobs.rs`; either
 *  side may resolve an unconfigured job, so they must agree. */
export const DEFAULT_JOB_MODELS: JobModels = {
  lectureChapters: [{ provider: "codex", model: "gpt-5.6-luna", reasoningEffort: "xhigh" }],
  lectureEnd: [{ provider: "claude", model: "claude-haiku-4-5-20251001", reasoningEffort: null }],
  threadNaming: [{ provider: "claude", model: "claude-haiku-4-5-20251001", reasoningEffort: null }],
  documentSuggestions: [{ provider: "claude", model: "claude-sonnet-5-5", reasoningEffort: "medium" }],
};

const JOB_MODELS_KEY = "job_models";
const JOB_NOTICES_KEY = "job_model_notices";

/** Fired on `window` after job models or notices are written. */
export const JOB_MODELS_CHANGED_EVENT = "oculus:job-models-changed";

function announce() {
  window.dispatchEvent(new CustomEvent(JOB_MODELS_CHANGED_EVENT));
}

/** One stored entry, or null when its model is blank or its provider unknown
 *  (checked against `PROVIDERS`, not a hardcoded list that falls behind). */
function parseSelection(row: unknown): JobSelection | null {
  if (!row || typeof row !== "object") return null;
  const r = row as Record<string, unknown>;
  if (typeof r.model !== "string" || !r.model.trim()) return null;
  if (!isProvider(r.provider)) return null;
  // Rust's serde rejects a level that is neither a string nor null/absent.
  const effort = r.reasoningEffort;
  if (effort !== undefined && effort !== null && typeof effort !== "string") return null;
  return { provider: r.provider, model: r.model, reasoningEffort: effort ?? null };
}

/** Same rules as `harness::jobs` in Rust. A missing key, or a legacy single
 *  object that doesn't parse, takes the default; an array is taken as stored,
 *  invalid entries dropped, so an emptied list stays off and never comes back
 *  as the default (which can be expensive). */
export async function getJobModels(): Promise<JobModels> {
  const raw = await getSetting(JOB_MODELS_KEY);
  if (!raw) return structuredClone(DEFAULT_JOB_MODELS);
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return structuredClone(DEFAULT_JOB_MODELS);
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return structuredClone(DEFAULT_JOB_MODELS);
  }
  const stored = parsed as Record<string, unknown>;
  const out = structuredClone(DEFAULT_JOB_MODELS);
  for (const job of JOBS) {
    const value = stored[job.id];
    if (Array.isArray(value)) {
      out[job.id] = value.map(parseSelection).filter((s): s is JobSelection => s !== null);
    } else if (value && typeof value === "object") {
      const one = parseSelection(value);
      if (one) out[job.id] = [one];
    }
  }
  return out;
}

/** The chains as they are written: entries with no model yet dropped. */
export function cleanJobModels(models: JobModels): JobModels {
  const out = {} as JobModels;
  for (const job of JOBS) {
    out[job.id] = (models[job.id] ?? []).filter((s) => s.model.trim() !== "");
  }
  return out;
}

export async function setJobModels(models: JobModels): Promise<void> {
  await setSetting(JOB_MODELS_KEY, JSON.stringify(cleanJobModels(models)));
  announce();
}

/** A configured model that left its agent's list and was removed from a
 *  job's chain, kept until the student dismisses it in Settings → Jobs. */
export interface JobNotice {
  job: JobId;
  provider: Provider;
  model: string;
  /** The model's display name when known, else its id. */
  label: string;
  /** ISO timestamp of the removal. */
  at: string;
}

function parseNotice(row: unknown): JobNotice | null {
  if (!row || typeof row !== "object") return null;
  const r = row as Record<string, unknown>;
  if (!isJobId(r.job) || !isProvider(r.provider)) return null;
  if (typeof r.model !== "string" || !r.model) return null;
  return {
    job: r.job,
    provider: r.provider,
    model: r.model,
    label: typeof r.label === "string" && r.label ? r.label : r.model,
    at: typeof r.at === "string" ? r.at : "",
  };
}

/** Malformed rows, and rows for jobs this build doesn't have, are dropped. */
export async function getJobNotices(): Promise<JobNotice[]> {
  const raw = await getSetting(JOB_NOTICES_KEY);
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.map(parseNotice).filter((n): n is JobNotice => n !== null);
  } catch {
    return [];
  }
}

async function writeNotices(list: JobNotice[]): Promise<void> {
  await setSetting(JOB_NOTICES_KEY, JSON.stringify(list));
  announce();
}

const noticeKey = (n: JobNotice) => `${n.job}\n${n.provider}\n${n.model}`;

/** Appends; a model removed again from the same job replaces its older notice. */
export async function addJobNotices(list: JobNotice[]): Promise<void> {
  if (list.length === 0) return;
  const fresh = new Set(list.map(noticeKey));
  const kept = (await getJobNotices()).filter((n) => !fresh.has(noticeKey(n)));
  await writeNotices([...kept, ...list]);
}

/** One job's notices, or every job's when `job` is omitted. */
export async function dismissJobNotices(job?: JobId): Promise<void> {
  const current = await getJobNotices();
  const kept = job ? current.filter((n) => n.job !== job) : [];
  if (kept.length === current.length) return;
  await writeNotices(kept);
}
