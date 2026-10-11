/** What the shell (`Onboarding.tsx`) hands every step. A step renders its
 *  body inside `StepFrame` and decides when Next is enabled; the shell owns
 *  the order, Back, the step count and what Skip records. */
export interface StepProps {
  onNext: () => void;
  onSkip: () => void;
}
