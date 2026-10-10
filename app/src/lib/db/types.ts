export interface Subject {
  id: number;
  code: string;
  name: string;
  term_name: string | null;
  is_current: boolean;
  workflow_state: string;
  selected: boolean;
  last_synced_at: string | null;
  created_at: string;
}

/** `scheduled` is never written now, but old `sync_runs` rows carry it. */
export type SyncOrigin = "manual" | "scheduled";

export interface SyncRun {
  id: number;
  started_at: string;
  finished_at: string | null;
  status: "running" | "completed" | "failed";
  subjects_synced: number;
  pages_scraped: number;
  error: string | null;
  /** JSON array of course codes the run targeted; NULL on old runs. */
  subject_codes: string | null;
  origin: SyncOrigin;
}

export type SyncFileAction = "new" | "updated" | "unchanged";

export interface SyncRunFile {
  id: number;
  run_id: number;
  subject_id: number | null;
  relative_path: string;
  action: SyncFileAction;
  size_bytes: number | null;
  timestamp: string;
  /** Joined from subjects; null if the subject row is gone. */
  subject_code: string | null;
}

/** A sync run plus its per-file ledger rolled up. */
export interface SyncRunSummary extends SyncRun {
  new_count: number;
  updated_count: number;
  unchanged_count: number;
  file_count: number;
}

export interface DbFile {
  id: number;
  subject_id: number;
  filename: string;
  relative_path: string;
  file_type: string;
  size_bytes: number | null;
  category: string | null;
  canvas_id: number | null;
  source_url: string | null;
  modified_at: string | null;
  scraped_at: string;
  parse_status: string | null;
  parsed_at: string | null;
  /** Sticky and model-blind — see `getUnembeddedPdfs` in lib/pipeline/retrieval.ts. */
  embed_status: string | null;
  embedded_at: string | null;
  /** Set once on first insert; NULL rows never show as "new". */
  first_seen_at: string | null;
  last_accessed_at: string | null;
  /** When a scrape last found the bytes new or changed (`scraped_at` bumps
   *  every run). Newer than `last_accessed_at` ⇒ the unseen dot returns. */
  content_changed_at: string | null;
}
