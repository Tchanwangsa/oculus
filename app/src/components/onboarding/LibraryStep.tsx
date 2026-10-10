import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

export function LibraryStep({ onNext, onSkip }: StepProps) {
  return (
    <StepFrame
      title="Set up your library"
      description="Add the keys that turn your PDFs into text and let the agent search pages."
      onNext={onNext}
      onSkip={onSkip}
    />
  );
}
