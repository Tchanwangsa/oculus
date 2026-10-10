import { getSetting, setSetting } from "./settings";
import { isProvider, type Provider } from "@/lib/harness/providers";

// Each non-chat model job names its own agent, model and reasoning level. One
// JSON value, read back in Rust by `harness::jobs`, where the jobs run.

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

export type JobModels = Record<JobId, JobSelection>;

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

/** Mirrors `default_selection` in `app/src-tauri/src/harness/jobs.rs`; either
 *  side may resolve an unconfigured job, so they must agree. */
export const DEFAULT_JOB_MODELS: JobModels = {
  lectureChapters: { provider: "codex", model: "gpt-5.6-luna", reasoningEffort: "xhigh" },
  lectureEnd: { provider: "claude", model: "claude-haiku-4-5-20251001", reasoningEffort: null },
  threadNaming: { provider: "claude", model: "claude-haiku-4-5-20251001", reasoningEffort: null },
  documentSuggestions: { provider: "claude", model: "claude-sonnet-5-5", reasoningEffort: "medium" },
};

const JOB_MODELS_KEY = "job_models";

/** A missing or malformed job row falls back to its default. */
export async function getJobModels(): Promise<JobModels> {
  const raw = await getSetting(JOB_MODELS_KEY);
  if (!raw) return structuredClone(DEFAULT_JOB_MODELS);
  try {
    const parsed = JSON.parse(raw);
    const out = structuredClone(DEFAULT_JOB_MODELS);
    for (const job of JOBS) {
      const row = parsed?.[job.id];
      if (!row || typeof row.model !== "string" || !row.model.trim()) continue;
      // Checked against `PROVIDERS`, not a hardcoded list that falls behind.
      if (!isProvider(row.provider)) continue;
      out[job.id] = {
        provider: row.provider,
        model: row.model,
        reasoningEffort: typeof row.reasoningEffort === "string" ? row.reasoningEffort : null,
      };
    }
    return out;
  } catch {
    return structuredClone(DEFAULT_JOB_MODELS);
  }
}

export async function setJobModels(models: JobModels): Promise<void> {
  await setSetting(JOB_MODELS_KEY, JSON.stringify(models));
}
