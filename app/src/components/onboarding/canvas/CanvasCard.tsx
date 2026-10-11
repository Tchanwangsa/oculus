import { CircleNotch, Student } from "@phosphor-icons/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { OptionCard } from "../OptionCard";
import type { CanvasState } from "./useCanvasConnection";

const CHIP: Record<CanvasState, { label: string; variant: "secondary" | "success" | "warning" | "default" } | null> = {
  checking: null,
  connected: { label: "Connected", variant: "success" },
  pending: { label: "Signing in…", variant: "default" },
  expired: { label: "Session expired", variant: "warning" },
  disconnected: { label: "Not connected", variant: "secondary" },
};

/** The one Canvas Oculus knows: the University of Melbourne's. `onConnect` is
 *  `useAuth().connect`, which tries the saved credentials before opening the
 *  sign-in window. */
export function CanvasCard({ state, onConnect }: { state: CanvasState; onConnect: () => void }) {
  const chip = CHIP[state];
  const action =
    state === "connected" ? (
      <Button variant="ghost" size="xs" onClick={onConnect}>
        Reconnect
      </Button>
    ) : state === "disconnected" || state === "expired" ? (
      <Button size="xs" onClick={onConnect}>
        {state === "expired" ? "Sign in again" : "Connect"}
      </Button>
    ) : null;

  return (
    <OptionCard
      icon={<Student weight="duotone" />}
      title="University of Melbourne Canvas"
      chip={chip && <Badge variant={chip.variant}>{chip.label}</Badge>}
      detail="canvas.lms.unimelb.edu.au"
      actions={action}
      selected={state === "connected"}
    >
      {state === "pending" && (
        <p className="flex items-center gap-2 text-xs text-muted-foreground">
          <CircleNotch size={13} className="shrink-0 animate-spin" />
          Finish signing in in the Canvas window.
        </p>
      )}
    </OptionCard>
  );
}
