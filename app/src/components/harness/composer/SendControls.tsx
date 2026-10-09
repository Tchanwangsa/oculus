import { PaperPlaneTilt, Stop } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";

/** Sending and queueing share the same readiness gate, including attachments. */
export function SendControls({ running, ready, writing, onSend, onStop }: {
  running: boolean;
  ready: boolean;
  writing: boolean;
  onSend: () => void;
  onStop: () => void;
}) {
  return (
    <>
      {running && (
        <Button size="icon-xs" variant="ghost" className="shrink-0" aria-label="Stop" onClick={onStop}>
          <Stop weight="fill" />
        </Button>
      )}
      {(!running || ready) && (
        <Button size="icon-xs" disabled={!ready || writing} onClick={onSend} className="shrink-0" aria-label={running ? "Queue" : "Send"}>
          <PaperPlaneTilt />
        </Button>
      )}
    </>
  );
}
