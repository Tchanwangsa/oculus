import { isStepId, type StepId } from "@/lib/onboarding/steps";
import { getSetting, setSetting } from "./settings";

/** The `onboarding` settings row (docs/onboarding.md). `completedAt: null`
 *  means a run is due: the first one, or one asked for from Settings. */
export interface OnboardingRow {
  completedAt: string | null;
  skipped: StepId[];
}

const ONBOARDING_KEY = "onboarding";

/** Null for an absent row and for a malformed one: both take the
 *  existing-setup check, so a damaged row never forces onboarding on a
 *  working setup. */
export function parseOnboardingRow(raw: string | null): OnboardingRow | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return null;
    const { completedAt, skipped } = parsed as Record<string, unknown>;
    if (completedAt !== null && typeof completedAt !== "string") return null;
    return {
      completedAt,
      skipped: Array.isArray(skipped) ? skipped.filter(isStepId) : [],
    };
  } catch {
    return null;
  }
}

export async function getOnboarding(): Promise<OnboardingRow | null> {
  return parseOnboardingRow(await getSetting(ONBOARDING_KEY));
}

export async function setOnboarding(row: OnboardingRow): Promise<void> {
  await setSetting(ONBOARDING_KEY, JSON.stringify(row));
}
