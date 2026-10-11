import { CanvasCard } from "./canvas/CanvasCard";
import { NeverSignInAgain } from "./canvas/NeverSignInAgain";
import { useCanvasConnection } from "./canvas/useCanvasConnection";
import { STEP_META } from "./stepMeta";
import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

/** Signs in to Canvas the way the shell does (`useAuth().connect`), with the
 *  optional automated sign-in folded below. Next waits for a session. */
export function CanvasStep({ onNext, onSkip }: StepProps) {
  const { state, connect } = useCanvasConnection();
  return (
    <StepFrame
      title="Connect Canvas"
      description="Sign in once with your university account so Oculus can sync your subjects."
      icon={STEP_META.canvas.icon}
      onNext={onNext}
      nextDisabled={state !== "connected"}
      onSkip={onSkip}
    >
      <div className="flex flex-col gap-5">
        <CanvasCard state={state} onConnect={() => void connect()} />
        <NeverSignInAgain />
      </div>
    </StepFrame>
  );
}
