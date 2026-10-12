import { CheckCircle, CircleNotch } from "@phosphor-icons/react";

import type { MarkStep } from "./useMarkFlow";
import { HeaderAction } from "../HeaderAction";

/** Ed's Mark: disabled offline (the reason as its tooltip) and while a mark
 *  is being checked or submitted. */
export function MarkButton({
  step,
  offline,
  onClick,
}: {
  step: MarkStep["step"];
  /** Why Ed can't be reached now; null when online. */
  offline: string | null;
  onClick: () => void;
}) {
  const busy = step !== "idle";
  const reason = offline ?? (busy ? "Marking…" : null);
  return (
    <HeaderAction variant="default" reason={reason} onClick={onClick}>
      {busy ? <CircleNotch className="animate-spin" aria-hidden /> : <CheckCircle aria-hidden />}
      {step === "marking" ? "Marking…" : "Mark"}
    </HeaderAction>
  );
}
