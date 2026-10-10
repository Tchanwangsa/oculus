import { useState, type ComponentType } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { STEP_IDS, type StepId } from "@/lib/onboarding/steps";
import { finishOnboarding } from "@/stores/shell/onboardingStore";
import { AgentStep } from "./AgentStep";
import { CanvasStep } from "./CanvasStep";
import { DoneStep } from "./DoneStep";
import { LibraryStep } from "./LibraryStep";
import { StepNavContext } from "./stepNav";
import type { StepProps } from "./types";
import { WelcomeStep } from "./WelcomeStep";

const STEPS: Record<StepId, ComponentType<StepProps>> = {
  welcome: WelcomeStep,
  canvas: CanvasStep,
  library: LibraryStep,
  agent: AgentStep,
  done: DoneStep,
};

/** Welcome and Done frame the count; only the steps between are numbered. */
const NUMBERED = STEP_IDS.slice(1, -1);

/**
 * First-run setup, rendered by `App` in place of the shell while it is open
 * (docs/onboarding.md), so the persisted tab strip is never touched. Owns the
 * step order and the skipped list; finishing writes both to the settings row.
 */
export function Onboarding() {
  const [index, setIndex] = useState(0);
  const [skipped, setSkipped] = useState<StepId[]>([]);
  const [error, setError] = useState<string | null>(null);
  const id = STEP_IDS[index];
  const Step = STEPS[id];
  const number = NUMBERED.indexOf(id);

  const advance = (skip: boolean) => {
    // Rebuilt in step order, so a step skipped and later finished drops out.
    const next = STEP_IDS.filter((s) => (s === id ? skip : skipped.includes(s)));
    setSkipped(next);
    if (index < STEP_IDS.length - 1) {
      setIndex(index + 1);
      return;
    }
    setError(null);
    finishOnboarding(next).catch((e) => setError(`Couldn't save setup: ${String(e)}`));
  };

  return (
    <TooltipProvider delayDuration={500}>
      <div data-select-scope className="flex h-full w-full flex-col bg-background">
        {/* Under the overlay title bar: drags the window like the tab strip. */}
        <div data-tauri-drag-region className="h-11 shrink-0" />
        <div className="flex flex-1 overflow-y-auto px-8 pb-8">
          <div className="m-auto w-full max-w-lg flex-none rounded-xl border border-border bg-card p-8 shadow-panel">
            <StepNavContext.Provider
              value={{
                number: number < 0 ? null : number + 1,
                total: NUMBERED.length,
                back: index > 0 ? () => setIndex(index - 1) : null,
              }}
            >
              <Step key={id} onNext={() => advance(false)} onSkip={() => advance(true)} />
            </StepNavContext.Provider>
            {error && (
              <p data-selectable className="mt-4 text-xs text-destructive">
                {error}
              </p>
            )}
          </div>
        </div>
      </div>
    </TooltipProvider>
  );
}
