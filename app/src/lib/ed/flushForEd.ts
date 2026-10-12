import type { WorkspaceSaver } from "@/lib/ed/workspaceSaver";

/**
 * Writes every pending edit before Run or Mark, which use Ed's latest commit:
 * resolves null when everything typed reached a save, else why not, so the
 * caller stops instead of running or marking older code.
 */
export async function flushForEd(saver: Pick<WorkspaceSaver, "flush" | "dirty" | "status">): Promise<string | null> {
  await saver.flush();
  // An edit typed while that write ran is still pending.
  if (saver.dirty()) await saver.flush();
  const status = saver.status;
  if (status.state === "error") return `Your latest changes aren't saved to Ed: ${status.message}`;
  if (saver.dirty()) return "Your latest changes are still saving. Try again in a moment.";
  return null;
}
