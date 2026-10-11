import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

export function WelcomeStep({ onNext }: StepProps) {
  return (
    <StepFrame
      title="Welcome to Oculus"
      description="Oculus syncs your Canvas subjects, Ed threads and lecture recordings into one library on this Mac, and a coding agent answers questions from it. Setup takes a few minutes."
      onNext={onNext}
      nextLabel="Get started"
    />
  );
}
