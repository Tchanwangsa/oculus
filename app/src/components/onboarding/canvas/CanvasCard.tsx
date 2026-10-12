import { useState } from "react";
import { CircleNotch, PencilSimple, Student } from "@phosphor-icons/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  signInSummary,
  useCanvasAddressForm,
} from "@/components/settings/web/useCanvasAddressForm";
import { useInstitution } from "@/stores/sync/institutionStore";
import { OptionCard } from "../OptionCard";
import type { CanvasState } from "./useCanvasConnection";

const CHIP: Record<CanvasState, { label: string; variant: "secondary" | "success" | "warning" | "default" } | null> = {
  checking: null,
  connected: { label: "Connected", variant: "success" },
  pending: { label: "Signing in…", variant: "default" },
  expired: { label: "Session expired", variant: "warning" },
  disconnected: { label: "Not connected", variant: "secondary" },
};

/** The student's Canvas: its address first, as in Settings → Canvas, then the
 *  sign-in. `onConnect` is `useAuth().connect`, which tries the saved
 *  credentials before opening the sign-in window; `error` is why that window
 *  could not open. */
export function CanvasCard({
  state,
  error,
  onConnect,
}: {
  state: CanvasState;
  error: string | null;
  onConnect: () => void;
}) {
  const institution = useInstitution();
  const [editing, setEditing] = useState(false);
  const form = useCanvasAddressForm(() => setEditing(false));
  const configured = !!institution?.canvas;
  const showField = institution !== undefined && (editing || !configured);
  const chip = configured && !editing ? CHIP[state] : null;

  const action = !configured || editing ? null : (
    <>
      {!institution.locked && state !== "pending" && (
        <Button
          variant="ghost"
          size="xs"
          onClick={() => {
            form.setAddress(institution.host ?? "");
            setEditing(true);
          }}
        >
          <PencilSimple size={13} /> Change
        </Button>
      )}
      {state === "connected" ? (
        <Button variant="ghost" size="xs" onClick={onConnect}>
          Reconnect
        </Button>
      ) : state === "disconnected" || state === "expired" ? (
        <Button size="xs" onClick={onConnect}>
          {state === "expired" ? "Sign in again" : "Connect"}
        </Button>
      ) : null}
    </>
  );

  return (
    <OptionCard
      icon={<Student weight="duotone" />}
      title="Your university's Canvas"
      chip={chip && <Badge variant={chip.variant}>{chip.label}</Badge>}
      detail={configured && !editing ? institution.host : "The address you open Canvas at"}
      actions={action}
      selected={configured && state === "connected"}
    >
      {showField ? (
        <div className="flex flex-col gap-2">
          <form
            className="flex items-center gap-1.5"
            onSubmit={(e) => {
              e.preventDefault();
              void form.save();
            }}
          >
            <Input
              className="h-7 text-xs"
              placeholder="canvas.youruni.edu or youruni.instructure.com"
              autoComplete="off"
              autoCapitalize="off"
              spellCheck={false}
              autoFocus
              value={form.address}
              onChange={(e) => form.setAddress(e.target.value)}
              disabled={form.busy}
            />
            <Button type="submit" size="xs" disabled={form.busy || !form.address.trim()}>
              {form.busy && <CircleNotch size={13} className="animate-spin" />}
              {form.busy ? "Checking…" : "Save"}
            </Button>
            {editing && (
              <Button
                type="button"
                variant="ghost"
                size="xs"
                disabled={form.busy}
                onClick={() => {
                  setEditing(false);
                  form.setError(null);
                }}
              >
                Cancel
              </Button>
            )}
          </form>
          {form.error ? (
            <p className="text-xs text-destructive">{form.error}</p>
          ) : (
            <p className="text-xs text-muted-foreground">
              Oculus checks it is Canvas and works out how your university signs in.
            </p>
          )}
        </div>
      ) : state === "pending" ? (
        <p className="flex items-center gap-2 text-xs text-muted-foreground">
          <CircleNotch size={13} className="shrink-0 animate-spin" />
          Finish signing in in the Canvas window.
        </p>
      ) : configured && error ? (
        <p className="text-xs text-destructive">{error}</p>
      ) : configured && state !== "connected" ? (
        <p className="text-xs text-muted-foreground">{signInSummary(institution)}</p>
      ) : null}
    </OptionCard>
  );
}
