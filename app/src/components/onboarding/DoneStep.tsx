import { Check } from "@phosphor-icons/react";
import { HeroHalo } from "./HeroHalo";
import { SetupOverview } from "./SetupOverview";
import { StepFrame } from "./StepFrame";
import { useStepNav } from "./stepNav";
import type { StepProps } from "./types";

export function DoneStep({ onNext }: StepProps) {
  const { skipped } = useStepNav();
  return (
    <StepFrame
      title="You're all set"
      description="You can run this setup again from Settings → Canvas."
      hero={
        <HeroHalo round>
          <span className="flex size-16 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-md">
            <Check size={30} weight="bold" />
          </span>
        </HeroHalo>
      }
      onNext={onNext}
      nextLabel="Open Oculus"
    >
      <SetupOverview skipped={skipped} />
    </StepFrame>
  );
}
