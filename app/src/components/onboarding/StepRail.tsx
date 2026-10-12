import { Fragment } from "react";
import { Check, Minus } from "@phosphor-icons/react";
import type { StepId } from "@/lib/onboarding/steps";
import { cn } from "@/lib/utils";
import { SETUP_STEPS, STEP_META, type SetupStepId } from "./stepMeta";

type RailState = "done" | "skipped" | "current" | "upcoming";

function stateOf(step: SetupStepId, current: SetupStepId, skipped: readonly StepId[]): RailState {
  const at = SETUP_STEPS.indexOf(step);
  const now = SETUP_STEPS.indexOf(current);
  if (at === now) return "current";
  if (at > now) return "upcoming";
  return skipped.includes(step) ? "skipped" : "done";
}

const DOT: Record<RailState, string> = {
  done: "border-primary bg-primary text-primary-foreground",
  skipped: "border-dashed border-input bg-card text-muted-foreground",
  current: "border-brand/40 bg-brand-muted text-brand ring-4 ring-brand/10",
  upcoming: "border-border bg-card text-muted-foreground",
};

/** The line into a step, keyed by that step's state: filled once the step
 *  before it is behind, a fade into the current one, plain ahead. */
const LINE: Record<RailState, string> = {
  done: "bg-primary/40",
  skipped: "bg-primary/40",
  current: "bg-linear-to-r from-primary/40 to-border",
  upcoming: "bg-border",
};

/** The three setup steps as a connected rail across the top of the card:
 *  a check for a finished step, a dash for a skipped one, the step's own
 *  glyph for the current and the ones to come. */
export function StepRail({
  current,
  skipped,
}: {
  current: SetupStepId;
  skipped: readonly StepId[];
}) {
  return (
    <ol className="flex items-center gap-3" aria-label="Setup progress">
      {SETUP_STEPS.map((step, i) => {
        const state = stateOf(step, current, skipped);
        const { label, icon: StepIcon } = STEP_META[step];
        const Glyph = state === "done" ? Check : state === "skipped" ? Minus : StepIcon;
        return (
          <Fragment key={step}>
            {i > 0 && (
              <li
                aria-hidden
                className={cn("h-px min-w-6 flex-1", LINE[state])}
              />
            )}
            <li
              className="flex shrink-0 items-center gap-2"
              aria-current={state === "current" ? "step" : undefined}
            >
              <span
                className={cn(
                  "flex size-7 items-center justify-center rounded-full border transition-[color,background-color,border-color,box-shadow]",
                  DOT[state],
                )}
              >
                <Glyph size={14} weight={state === "done" ? "bold" : "regular"} />
              </span>
              <span
                className={cn(
                  "text-[13px]",
                  state === "current" ? "font-medium text-foreground" : "text-muted-foreground",
                )}
              >
                {label}
                {state === "skipped" && <span className="sr-only"> (skipped)</span>}
              </span>
            </li>
          </Fragment>
        );
      })}
    </ol>
  );
}
