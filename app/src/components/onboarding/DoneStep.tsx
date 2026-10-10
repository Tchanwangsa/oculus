import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

export function DoneStep({ onNext }: StepProps) {
  return (
    <StepFrame
      title="You're all set"
      description="You can run this setup again from Settings → Canvas."
      onNext={onNext}
      nextLabel="Open Oculus"
    />
  );
}
