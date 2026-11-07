import { CircleNotch } from "@phosphor-icons/react";
import { Pill } from "@/components/markdown/openui/components/Frame";
import type { AskControls } from "./useWidgetAsk";
import { ASK_CAP } from "./limits";

/** The question a held request waits on, or the cap's offer: the words and
 *  pills alone, for the inline card's line and the expanded view's bar. */
export function AskChoice({ ask }: { ask: AskControls }) {
  if (ask.holding) {
    return (
      <>
        <span className="min-w-0 flex-1 truncate">This widget wants to use a model</span>
        <Pill tone="primary" onClick={ask.allow}>Allow</Pill>
        <Pill onClick={ask.deny}>Don't allow</Pill>
      </>
    );
  }
  if (ask.capped) {
    return (
      <>
        <span className="min-w-0 flex-1 truncate">This widget has used its {ASK_CAP} requests</span>
        <Pill onClick={ask.allowMore}>Allow {ASK_CAP} more</Pill>
      </>
    );
  }
  return null;
}

/** One line under the title while the widget's request needs the student. */
export function AskLine({ ask }: { ask: AskControls }) {
  if (!ask.holding && !ask.capped) return null;
  return (
    <div className="flex min-w-0 items-center gap-2 border-b border-border px-3 py-1.5 text-xs text-foreground">
      <AskChoice ask={ask} />
    </div>
  );
}

/** While an `ask` is with the model: a spinner and Stop. */
export function AskRunning({ ask }: { ask: AskControls }) {
  if (!ask.asking) return null;
  return (
    <>
      <CircleNotch size={13} aria-label="Asking a model" className="mx-1 shrink-0 animate-spin text-muted-foreground" />
      <Pill tone="ghost" onClick={ask.stop}>Stop</Pill>
    </>
  );
}
