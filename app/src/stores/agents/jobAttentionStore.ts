import { create } from "zustand";
import type { JobId, JobNotice } from "@/lib/db";
import type { Provider } from "@/lib/harness/providers";

/** A job's model whose agent CLI isn't found. Kept in the chain — the CLI may
 *  only be off PATH — so it is live state, never stored. */
export interface FlaggedJobModel {
  job: JobId;
  provider: Provider;
  model: string;
}

interface JobAttentionState {
  /** Removed models the student hasn't dismissed yet. */
  notices: JobNotice[];
  flagged: FlaggedJobModel[];
  /** Either list is non-empty: Settings and its Jobs row show a dot. */
  attention: boolean;
}

/** What `useJobAudit` (mounted once in `AppLayout`) found, for the badges. */
export const useJobAttention = create<JobAttentionState>(() => ({
  notices: [],
  flagged: [],
  attention: false,
}));
