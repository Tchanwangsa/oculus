import { ArrowUpIcon, CircleNotch, ListPlus, Play, Stop } from "@phosphor-icons/react";
import type { MouseEvent } from "react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { SendControl, StopControl } from "@/lib/harness";

/**
 * The composer's Stop and main button, as `runControls` decides them (Chat,
 * Studio and the lecture dock share it). Stop is an icon while it can be
 * pressed and says "Stopping" in words, inert, once it has been. The main
 * button sends or steers as an arrow, queues as a list-plus, or resumes a
 * held queue as a pill that says so. While a turn runs its tooltip names
 * the action (and a restart says so), and the icon follows ⌘ as it is held,
 * so the button shows what a press will do before it happens. Every kind
 * shares the readiness gate, attachments included.
 */
export function SendControls({ stop, send, onStop, onSend, onResume }: {
  stop: StopControl | null;
  send: SendControl | null;
  onStop: () => void;
  /** `e` says whether ⌘ was held for the click. */
  onSend: (e?: MouseEvent) => void;
  onResume: () => void;
}) {
  return (
    <>
      {stop &&
        (stop.stopping ? (
          <Button size="xs" variant="ghost" disabled className="shrink-0 text-muted-foreground">
            <CircleNotch weight="bold" className="animate-spin text-brand" aria-hidden />
            {stop.label}
          </Button>
        ) : (
          <Button size="icon-xs" variant="ghost" className="shrink-0" aria-label={stop.label} onClick={onStop}>
            <Stop weight="fill" />
          </Button>
        ))}
      {send?.kind === "resume" && (
        <Button size="xs" disabled={send.disabled} onClick={onResume} className="shrink-0 will-change-[opacity]">
          <Play weight="fill" aria-hidden />
          {send.label}
        </Button>
      )}
      {send && send.kind !== "resume" && <SendButton send={send} onSend={onSend} />}
    </>
  );
}

function SendButton({ send, onSend }: { send: SendControl; onSend: (e?: MouseEvent) => void }) {
  const button = (
    <Button
      size="icon-xs"
      disabled={send.disabled}
      onClick={(e) => onSend(e)}
      className="shrink-0 will-change-[opacity]"
      aria-label={send.label}
      aria-description={send.hint}
    >
      {send.kind === "queue" ? <ListPlus weight="bold" /> : <ArrowUpIcon />}
    </Button>
  );
  // Idle sends need no explanation; into a running turn the two actions
  // look alike, so the tooltip says which this is.
  if (send.kind === "send") return button;
  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent>
        <span className="block">{send.label}</span>
        {send.hint && <span className="block opacity-70">{send.hint}</span>}
      </TooltipContent>
    </Tooltip>
  );
}
