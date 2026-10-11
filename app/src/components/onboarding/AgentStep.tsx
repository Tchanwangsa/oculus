import { PROVIDERS, providerAvailability } from "@/lib/harness";
import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

export function AgentStep({ onNext, onSkip }: StepProps) {
  const providers = PROVIDERS.filter((provider) => providerAvailability(provider.id).supported);
  const labels = providers.map((provider) => provider.label);
  const names = `${labels.slice(0, -1).join(", ")} or ${labels[labels.length - 1]}`;
  return (
    <StepFrame
      title="Choose an agent"
      description={`Use ${names} from your machine to answer your questions.`}
      onNext={onNext}
      onSkip={onSkip}
    >
      {providers.filter((provider) => !providerAvailability(provider.id).install).map((provider) => (
        <p key={provider.id} className="text-xs text-muted-foreground">
          Set up {provider.label} in a terminal.
        </p>
      ))}
    </StepFrame>
  );
}
