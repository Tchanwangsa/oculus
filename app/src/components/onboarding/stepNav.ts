import { createContext, useContext } from "react";
import type { StepId } from "@/lib/onboarding/steps";

/** The shell's half of a step's frame, so `StepProps` stays two callbacks. */
export interface StepNav {
  /** Null on the first step. */
  back: (() => void) | null;
  /** The steps skipped so far, in step order — Done's summary reads it. */
  skipped: readonly StepId[];
}

export const StepNavContext = createContext<StepNav>({ back: null, skipped: [] });

export function useStepNav(): StepNav {
  return useContext(StepNavContext);
}
