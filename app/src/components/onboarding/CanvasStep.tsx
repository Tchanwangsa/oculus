import { CanvasCard } from "./canvas/CanvasCard";
import { NeverSignInAgain } from "./canvas/NeverSignInAgain";
import { useCanvasConnection } from "./canvas/useCanvasConnection";
import { STEP_META } from "./stepMeta";
import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

/** Takes the university's Canvas address, then signs in the way the shell
 *  does (`useAuth().connect`), with the optional automated sign-in folded
 *  below at an Okta university. Next waits for a session. */
export function CanvasStep({ onNext, onSkip }: StepProps) {
  const { state, error, connect } = useCanvasConnection();
  return (
    <StepFrame
      title="Connect Canvas"
      description="Enter your university's Canvas address and sign in once so Oculus can sync your subjects."
      icon={STEP_META.canvas.icon}
      onNext={onNext}
      nextDisabled={state !== "connected"}
      onSkip={onSkip}
    >
      <div className="flex flex-col gap-5">
        <CanvasCard state={state} error={error} onConnect={() => void connect()} />
        <NeverSignInAgain />
      </div>
    </StepFrame>
  );
}
