import { useState, type ComponentType } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { STEP_IDS, type StepId } from "@/lib/onboarding/steps";
import { finishOnboarding } from "@/stores/shell/onboardingStore";
import { AgentStep } from "./AgentStep";
import { Backdrop } from "./Backdrop";
import { CanvasStep } from "./CanvasStep";
import { DoneStep } from "./DoneStep";
import { LibraryStep } from "./LibraryStep";
import { isSetupStep } from "./stepMeta";
import { StepNavContext } from "./stepNav";
import { StepRail } from "./StepRail";
import type { StepProps } from "./types";
import { WelcomeStep } from "./WelcomeStep";

const STEPS: Record<StepId, ComponentType<StepProps>> = {
  welcome: WelcomeStep,
  canvas: CanvasStep,
  library: LibraryStep,
  agent: AgentStep,
  done: DoneStep,
};

/**
 * First-run setup, rendered by `App` in place of the shell while it is open
 * (docs/onboarding.md), so the persisted tab strip is never touched. Owns the
 * step order and the skipped list; finishing writes both to the settings row.
 * Draws the backdrop and the card; the progress rail heads the card on the
 * three setup steps, and Welcome and Done go without it.
 */
export function Onboarding() {
  const [index, setIndex] = useState(0);
  const [skipped, setSkipped] = useState<StepId[]>([]);
  const [error, setError] = useState<string | null>(null);
  const id = STEP_IDS[index];
  const Step = STEPS[id];

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
      <div data-select-scope className="relative flex h-full w-full flex-col bg-background">
        <Backdrop />
        {/* Under the overlay title bar: drags the window like the tab strip. */}
        <div data-tauri-drag-region className="relative h-11 shrink-0" />
        <div className="relative flex flex-1 overflow-y-auto px-8 pb-10">
          <div className="m-auto w-full max-w-2xl flex-none">
            <div className="overflow-hidden rounded-xl border border-border bg-card shadow-panel">
              {isSetupStep(id) && (
                <div className="border-b border-border-subtle px-8 py-4">
                  <StepRail current={id} skipped={skipped} />
                </div>
              )}
              <StepNavContext.Provider
                value={{ back: index > 0 ? () => setIndex(index - 1) : null, skipped }}
              >
                {/* Keyed, so each step fades in once; `will-change` keeps the
                    layer for good, or icons jump half a pixel when it drops
                    at a fractional page zoom (docs/ui.md). */}
                <div
                  key={id}
                  className="animate-in duration-300 ease-out fade-in-0 slide-in-from-bottom-1 will-change-[opacity,transform] motion-reduce:animate-none"
                >
                  <Step onNext={() => advance(false)} onSkip={() => advance(true)} />
                </div>
              </StepNavContext.Provider>
            </div>
            {error && (
              <p data-selectable className="mt-3 text-center text-xs text-destructive">
                {error}
              </p>
            )}
          </div>
        </div>
      </div>
    </TooltipProvider>
  );
}
