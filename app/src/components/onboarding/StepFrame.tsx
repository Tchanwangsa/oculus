import type { ReactNode } from "react";
import { ArrowLeft } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { useStepNav } from "./stepNav";

/** The chrome every step shares: the quiet step count, the title and
 *  description, the step's own body, and the Back / Skip / Next footer. */
export function StepFrame({
  title,
  description,
  children,
  onNext,
  nextLabel = "Next",
  nextDisabled = false,
  onSkip,
}: {
  title: string;
  description: ReactNode;
  children?: ReactNode;
  onNext: () => void;
  nextLabel?: string;
  nextDisabled?: boolean;
  /** Omitted on a step that can't be skipped (Welcome, Done). */
  onSkip?: () => void;
}) {
  const { number, total, back } = useStepNav();
  return (
    <div className="flex flex-col gap-6">
      <div>
        {number != null && (
          <p className="mb-2 text-xs text-muted-foreground tabular-nums">
            Step {number} of {total}
          </p>
        )}
        <h1 className="text-xl font-semibold text-foreground">{title}</h1>
        <p className="mt-1.5 text-[13px] text-muted-foreground">{description}</p>
      </div>
      {children}
      <div className="flex items-center gap-2">
        {back && (
          <Button variant="ghost" size="sm" onClick={back}>
            <ArrowLeft /> Back
          </Button>
        )}
        <div className="ml-auto flex items-center gap-2">
          {onSkip && (
            <Button variant="ghost" size="sm" onClick={onSkip}>
              Skip
            </Button>
          )}
          <Button size="sm" onClick={onNext} disabled={nextDisabled}>
            {nextLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
