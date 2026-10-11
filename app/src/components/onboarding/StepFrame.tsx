import type { ReactNode } from "react";
import { ArrowLeft, type Icon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { useStepNav } from "./stepNav";

/** The chrome every step shares, filling the card below the progress rail:
 *  the header (a glyph tile beside the title, or a centred hero on the
 *  bookends), the step's own body, and the Back / Skip / Next footer band. */
export function StepFrame({
  title,
  description,
  icon: StepIcon,
  hero,
  children,
  onNext,
  nextLabel = "Next",
  nextDisabled = false,
  onSkip,
}: {
  title: string;
  description: ReactNode;
  /** The step's glyph, drawn in an indigo tile beside the title. */
  icon?: Icon;
  /** Art centred above the title in place of `icon` (Welcome, Done); centres
   *  the header too. */
  hero?: ReactNode;
  children?: ReactNode;
  onNext: () => void;
  nextLabel?: string;
  nextDisabled?: boolean;
  /** Omitted on a step that can't be skipped (Welcome, Done). */
  onSkip?: () => void;
}) {
  const { back } = useStepNav();
  return (
    <div className="flex flex-col">
      <div className="flex flex-col gap-7 px-8 pt-8 pb-8">
        {hero ? (
          <div className="flex flex-col items-center text-center">
            {hero}
            <h1 className="mt-6 text-display-sm font-semibold text-foreground">{title}</h1>
            <p className="mt-2 max-w-md text-[13px]/relaxed text-muted-foreground">{description}</p>
          </div>
        ) : (
          <div className="flex items-start gap-4">
            {StepIcon && (
              <span className="flex size-11 shrink-0 items-center justify-center rounded-xl border border-brand/15 bg-brand-muted text-brand">
                <StepIcon size={22} weight="duotone" />
              </span>
            )}
            <div className="min-w-0 pt-0.5">
              <h1 className="text-xl font-semibold text-foreground">{title}</h1>
              <p className="mt-1 max-w-prose text-[13px]/relaxed text-muted-foreground">
                {description}
              </p>
            </div>
          </div>
        )}
        {children}
      </div>
      <div className="flex items-center gap-2 border-t border-border-subtle bg-surface/50 px-8 py-4">
        {back && (
          <Button variant="ghost" size="sm" onClick={back}>
            <ArrowLeft /> Back
          </Button>
        )}
        <div className="ml-auto flex items-center gap-2">
          {onSkip && (
            <Button variant="ghost" size="sm" onClick={onSkip}>
              Skip for now
            </Button>
          )}
          <Button size="sm" className="min-w-24" onClick={onNext} disabled={nextDisabled}>
            {nextLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
