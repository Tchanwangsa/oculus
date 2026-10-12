import { ProviderMark } from "@/components/icons/ProviderMark";
import { PROVIDERS } from "@/lib/harness";
import { HeroHalo } from "./HeroHalo";
import { SetupOverview } from "./SetupOverview";
import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

export function WelcomeStep({ onNext }: StepProps) {
  return (
    <StepFrame
      title="Welcome to Oculus"
      description="Oculus syncs your Canvas subjects, Ed threads and lecture recordings into one library on your computer, and a coding agent answers questions from it. Setup takes a few minutes."
      hero={
        <HeroHalo>
          <img
            src="/oculus-mark.svg"
            alt=""
            draggable={false}
            className="size-16 rounded-[22.5%] shadow-md"
          />
        </HeroHalo>
      }
      onNext={onNext}
      nextLabel="Get started"
    >
      <SetupOverview />
      <div className="flex flex-col items-center gap-3">
        <p className="text-xs text-muted-foreground">Bring the agent you already use</p>
        <ul className="flex flex-wrap justify-center gap-2">
          {PROVIDERS.map((p) => (
            <li
              key={p.id}
              className="flex h-8 items-center gap-2 rounded-full border border-border bg-card px-3.5 text-[13px] text-foreground shadow-xs"
            >
              <ProviderMark provider={p.id} className="size-4 shrink-0" />
              {p.label}
            </li>
          ))}
        </ul>
      </div>
    </StepFrame>
  );
}
