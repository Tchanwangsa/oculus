export const PHASE_LABEL: Record<string, string> = {
  home: "overview",
  announcements: "announcements",
  modules: "modules",
};

export type ActivityView = "history" | "pipeline";
export const VIEW_KEY = "oculus-sync-view";
/** How long a failed pipeline seed waits before trying again. */
export const SEED_RETRY_MS = 5_000;

/** The page's two tables, as sibling tabs. */
export const VIEWS = [
  { value: "history", label: "Sync History" },
  { value: "pipeline", label: "File Activity" },
] as const satisfies ReadonlyArray<{ value: ActivityView; label: string }>;
