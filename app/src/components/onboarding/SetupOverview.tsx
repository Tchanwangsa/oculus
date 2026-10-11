import { Check, Minus } from "@phosphor-icons/react";
import type { StepId } from "@/lib/onboarding/steps";
import { cn } from "@/lib/utils";
import { SETUP_STEPS, STEP_META } from "./stepMeta";

/** The three setup steps as tiles: Welcome previews them; Done passes
 *  `skipped` and each tile says whether it was set up or skipped. */
export function SetupOverview({ skipped }: { skipped?: readonly StepId[] }) {
  return (
    <ul className="grid grid-cols-3 gap-3">
      {SETUP_STEPS.map((step) => {
        const { label, icon: StepIcon, blurb } = STEP_META[step];
        const status = skipped ? (skipped.includes(step) ? "skipped" : "done") : null;
        return (
          <li
            key={step}
            className={cn(
              "flex flex-col rounded-xl border p-4",
              status === "skipped"
                ? "border-dashed border-input bg-card"
                : "border-border-subtle bg-surface/60",
            )}
          >
            <div className="flex items-center justify-between">
              <span
                className={cn(
                  "flex size-9 items-center justify-center rounded-lg border",
                  status === "skipped"
                    ? "border-border-subtle bg-surface text-muted-foreground"
                    : "border-brand/15 bg-brand-muted text-brand",
                )}
              >
                <StepIcon size={18} weight="duotone" />
              </span>
              {status === "done" && (
                <span className="flex size-5 items-center justify-center rounded-full bg-primary text-primary-foreground">
                  <Check size={11} weight="bold" />
                  <span className="sr-only">Set up</span>
                </span>
              )}
              {status === "skipped" && (
                <span className="flex items-center gap-1 text-xs text-muted-foreground">
                  <Minus size={12} /> Skipped
                </span>
              )}
            </div>
            <div className="mt-3 text-[13px] font-medium text-foreground">{label}</div>
            <p className="mt-0.5 text-xs/relaxed text-muted-foreground">{blurb}</p>
          </li>
        );
      })}
    </ul>
  );
}
