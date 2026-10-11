/** The agent CLIs the app can drive. A leaf with no imports, so `lib/db` can
 *  validate a stored provider without pulling in the harness. */
export const PROVIDER_IDS = ["claude", "codex", "opencode", "antigravity"] as const;

export type Provider = (typeof PROVIDER_IDS)[number];

/** Narrow a stored string to a provider this build has. */
export function isProvider(value: unknown): value is Provider {
  return typeof value === "string" && (PROVIDER_IDS as readonly string[]).includes(value);
}
