import { ArrowUpIcon, CircleNotch, Play, Stop } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import type { SendControl, StopControl } from "@/lib/harness";

/**
 * The composer's Stop and main button, as `runControls` decides them (Chat,
 * Studio and the lecture dock share it). Stop is an icon while it can be
 * pressed and says "Stopping" in words, inert, once it has been. The main
 * button sends or queues as an arrow, or resumes a held queue as a pill
 * that says so. Sending and queueing share the readiness gate, attachments
 * included.
 */
export function SendControls({ stop, send, onStop, onSend, onResume }: {
  stop: StopControl | null;
  send: SendControl | null;
  onStop: () => void;
  onSend: () => void;
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
      {send && send.kind !== "resume" && (
        <Button size="icon-xs" disabled={send.disabled} onClick={onSend} className="shrink-0" aria-label={send.label}>
          <ArrowUpIcon />
        </Button>
      )}
    </>
  );
}
