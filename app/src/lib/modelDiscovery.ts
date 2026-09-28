/** One local CLI catalogue request. Health snapshots change only after a
 * successful shared recheck, so an empty answer can recover without polling. */
export interface ModelDiscoveryAttempt {
  health: object | null;
  version: number;
  status: "pending" | "empty" | "ready";
}

export function shouldDiscoverModels(
  attempt: ModelDiscoveryAttempt | undefined,
  health: object | null,
  version: number,
): boolean {
  if (!attempt) return true;
  if (attempt.status === "pending") return false;
  return attempt.version !== version
    || (attempt.status === "empty" && attempt.health !== health);
}
