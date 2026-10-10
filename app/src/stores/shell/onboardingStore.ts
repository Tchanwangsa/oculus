import { create } from "zustand";
import { setOnboarding } from "@/lib/db/onboarding";
import type { StepId } from "@/lib/onboarding/steps";

/** Whether onboarding replaces the shell. Null until the launch gate
 *  (`hooks/shell/useOnboardingGate.ts`) decides; a store so Settings can
 *  reopen it without a reload. */
interface OnboardingState {
  open: boolean | null;
}

export const useOnboardingStore = create<OnboardingState>(() => ({ open: null }));

/** Settings' "Run setup again": a row with `completedAt: null` reopens it on
 *  the next launch too, until it is finished. */
export async function reopenOnboarding(): Promise<void> {
  await setOnboarding({ completedAt: null, skipped: [] });
  useOnboardingStore.setState({ open: true });
}

export async function finishOnboarding(skipped: StepId[]): Promise<void> {
  await setOnboarding({ completedAt: new Date().toISOString(), skipped });
  useOnboardingStore.setState({ open: false });
}
