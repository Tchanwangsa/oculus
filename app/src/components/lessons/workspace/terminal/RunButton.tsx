import { CircleNotch, Play, Stop } from "@phosphor-icons/react";

import type { RunPhase } from "./useRunSession";
import { HeaderAction } from "../HeaderAction";

/** Run, and Stop while a run is going. Disabled offline, the reason as its
 *  tooltip. */
export function RunButton({
  phase,
  offline,
  onRun,
  onStop,
}: {
  phase: RunPhase;
  offline: string | null;
  onRun: () => void;
  onStop: () => void;
}) {
  if (phase === "running" || phase === "stopping") {
    return (
      <HeaderAction reason={phase === "stopping" ? "Stopping…" : null} onClick={onStop}>
        <Stop weight="fill" aria-hidden />
        Stop
      </HeaderAction>
    );
  }
  const starting = phase === "starting";
  return (
    <HeaderAction reason={offline ?? (starting ? "Starting…" : null)} onClick={onRun}>
      {starting ? <CircleNotch className="animate-spin" aria-hidden /> : <Play weight="fill" aria-hidden />}
      Run
    </HeaderAction>
  );
}
