import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

export function AgentStep({ onNext, onSkip }: StepProps) {
  return (
    <StepFrame
      title="Choose an agent"
      description="Install and sign in to the coding agent that answers your questions."
      onNext={onNext}
      onSkip={onSkip}
    />
  );
}
