import { useState } from "react";
import { CircleNotch, Info, PencilSimple } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  useInstitution,
  useInstitutionStore,
  type Institution,
} from "@/stores/institutionStore";

/** How the saved address signs in, in a few words. */
function signInSummary(i: Institution): string {
  if (i.okta && i.sso_host) return `Signs in through Okta at ${i.sso_host}.`;
  if (i.sso_host) return `Signs in through ${i.sso_host}.`;
  return "Signs in on Canvas's own login page.";
}

/**
 * The university's Canvas address — the one thing a student types; Rust
 * checks it is Canvas and detects how it signs in. A synced library pins it
 * until Start over, since course ids mean nothing on another Canvas.
 */
export function CanvasAddress() {
  const institution = useInstitution();
  const configure = useInstitutionStore((s) => s.configure);
  const [editing, setEditing] = useState(false);
  const [address, setAddress] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (institution === undefined) return null;
  const showField = editing || !institution.canvas;

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await configure(address);
      setEditing(false);
      setAddress("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="pb-1">
      <div className="flex items-center justify-between gap-4 py-2">
        <Label className="text-xs font-normal text-foreground flex items-center gap-1.5 shrink-0">
          Canvas address
          <Tooltip>
            <TooltipTrigger asChild>
              <Info size={12} className="text-muted-foreground/60" />
            </TooltipTrigger>
            <TooltipContent className="max-w-[260px]">
              The address you open Canvas at, such as canvas.youruni.edu or
              youruni.instructure.com. Oculus works out how your university
              signs in from it.
            </TooltipContent>
          </Tooltip>
        </Label>
        {showField ? (
          <form
            className="flex items-center gap-1.5 min-w-0 flex-1 justify-end"
            onSubmit={(e) => {
              e.preventDefault();
              if (address.trim() && !busy) save();
            }}
          >
            <Input
              className="h-7 text-xs max-w-64"
              placeholder="canvas.youruni.edu"
              autoComplete="off"
              autoCapitalize="off"
              spellCheck={false}
              autoFocus={editing}
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              disabled={busy}
            />
            <Button type="submit" size="xs" disabled={busy || !address.trim()}>
              {busy && <CircleNotch size={13} className="animate-spin" />}
              {busy ? "Checking…" : "Save"}
            </Button>
            {editing && (
              <Button
                type="button"
                variant="ghost"
                size="xs"
                disabled={busy}
                onClick={() => {
                  setEditing(false);
                  setError(null);
                }}
              >
                Cancel
              </Button>
            )}
          </form>
        ) : (
          <div className="flex items-center gap-1.5 min-w-0">
            <span className="text-xs text-muted-foreground truncate">{institution.host}</span>
            {institution.locked ? (
              <Tooltip>
                <TooltipTrigger asChild>
                  {/* A disabled button fires no pointer events, so the span holds the tooltip. */}
                  <span>
                    <Button variant="ghost" size="xs" disabled>
                      <PencilSimple size={13} /> Change
                    </Button>
                  </span>
                </TooltipTrigger>
                <TooltipContent className="max-w-[240px]">
                  Your library was synced from this Canvas. Start over in
                  Settings → Storage to switch university.
                </TooltipContent>
              </Tooltip>
            ) : (
              <Button
                variant="ghost"
                size="xs"
                onClick={() => {
                  setAddress(institution.host ?? "");
                  setEditing(true);
                }}
              >
                <PencilSimple size={13} /> Change
              </Button>
            )}
          </div>
        )}
      </div>
      {institution.canvas && !showField && (
        <p className="text-xs text-muted-foreground pb-1">{signInSummary(institution)}</p>
      )}
      {!institution.canvas && !error && (
        <p className="text-xs text-muted-foreground pb-1">
          Enter your university's Canvas address to connect.
        </p>
      )}
      {error && <p className="text-xs text-destructive pb-1">{error}</p>}
    </div>
  );
}
