/** The onboarding steps, in order. A leaf with no imports, so `lib/db` can
 *  validate a stored `skipped` list without pulling in the UI. */
export const STEP_IDS = ["welcome", "canvas", "library", "agent", "done"] as const;

export type StepId = (typeof STEP_IDS)[number];

export function isStepId(value: unknown): value is StepId {
  return typeof value === "string" && (STEP_IDS as readonly string[]).includes(value);
}
