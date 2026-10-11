import type { EngineOption } from "../../shared/EngineSelect";

/**
 * Mirrors `VoyageUsage` in `app/src-tauri/src/embed/commands.rs`. Oculus's own
 * tally (`voyage-usage.json`), not Voyage's — Voyage has no usage endpoint.
 */
export interface VoyageUsage {
  /** `"free"`, `"paid"` or `"unknown"` — see `plan_source` before believing it. */
  plan: string;
  plan_source: string;
  rpm: number;
  tpm: number;
  learned_at: number;
  requests: number;
  tokens: number;
  pixels: number;
  free_pixels: number;
  free_pixels_left: number;
  usd_per_billion_pixels: number;
  /** The spend guard, as a percentage of the free grant. 0 is off. */
  stop_at_percent: number;
  quota_latched: boolean;
}

/** Mirrors `Bucket` in `app/src-tauri/src/embed/estimate.rs`. */
export interface Bucket {
  label: string;
  files: number;
  pages: number;
}

/** Mirrors `EmbedEstimate` in `app/src-tauri/src/embed/estimate.rs`. */
export interface EmbedEstimate {
  files: number;
  pages: number;
  unreadable: number;
  pixels: number;
  tokens: number;
  requests: number;
  kinds: Bucket[];
  billable_pixels: number;
  cost_usd: number;
  free_pixels_left: number;
  /** Where the spend guard would cut this run short, if it would. */
  stops_after_pages: number | null;
  seconds: number;
  seconds_tier1: number;
  tier_rpm: number;
  tier_tpm: number;
  tier_free: boolean;
  tier_source: string;
}

/** Mirrors `EmbedSettings` in `app/src-tauri/src/embed/commands.rs`. */
export interface EmbedSettings {
  engine: string;
  base_url: string;
  model: string;
  dim: number;
  credentials_ready: boolean;
  /** The keychain refused the read; the key may still be saved. */
  credentials_error: string | null;
  engines: EngineOption[];
  index: {
    files_embedded: number;
    pages_embedded: number;
    pages_with_markdown: number;
    model: string | null;
    dim: number | null;
    files_stored: number;
    pages_stored: number;
    pages_stale: number;
    stale_models: string[];
  };
  /** `null` for a local engine: no allowance, no tier, nothing to guard. */
  usage: VoyageUsage | null;
}
