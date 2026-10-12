import { CircleNotch, Warning } from "@phosphor-icons/react";

import { HeaderAction } from "../HeaderAction";
import type { Keep } from "./useConflictResolve";

/** The warning strip over a file in conflict, with Ed's three choices.
 *  Keep mine and Take Ed's need Ed, so offline they say why not. */
export function ConflictBar({
  comparing,
  busy,
  offline,
  onCompare,
  onKeepMine,
  onTakeEds,
}: {
  comparing: boolean;
  busy: Keep | null;
  /** Why Ed can't be reached now; null when online. */
  offline: string | null;
  onCompare: () => void;
  onKeepMine: () => void;
  onTakeEds: () => void;
}) {
  const reason = offline ?? (busy ? "Resolving…" : null);
  const spinner = <CircleNotch className="animate-spin" aria-hidden />;
  return (
    <div
      role="alert"
      className="flex shrink-0 items-center gap-2 border-b border-warning/20 bg-warning/10 px-3 py-1.5 text-xs text-warning"
    >
      <Warning weight="fill" className="size-3.5 shrink-0" aria-hidden />
      <span className="min-w-0 flex-1 truncate">Ed has a different version of this file</span>
      <HeaderAction reason={null} onClick={onCompare}>
        {comparing ? "Back to editor" : "Compare"}
      </HeaderAction>
      <HeaderAction reason={reason} onClick={onKeepMine}>
        {busy === "mine" && spinner}
        Keep mine
      </HeaderAction>
      <HeaderAction reason={reason} onClick={onTakeEds}>
        {busy === "ed" && spinner}
        Take Ed's
      </HeaderAction>
    </div>
  );
}
