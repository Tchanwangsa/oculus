import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

export function CanvasStep({ onNext, onSkip }: StepProps) {
  return (
    <StepFrame
      title="Connect Canvas"
      description="Sign in to Canvas so Oculus can sync your subjects."
      onNext={onNext}
      onSkip={onSkip}
    />
  );
}
