import { createContext, useContext } from "react";

/** The shell's half of a step's frame, so `StepProps` stays two callbacks. */
export interface StepNav {
  /** 1-based among the numbered steps; null on Welcome and Done. */
  number: number | null;
  total: number;
  /** Null on the first step. */
  back: (() => void) | null;
}

export const StepNavContext = createContext<StepNav>({ number: null, total: 0, back: null });

export function useStepNav(): StepNav {
  return useContext(StepNavContext);
}
