import { CircleNotch, Info, SignIn, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { useAuth, type AuthStatus } from "@/hooks/useAuth";
import { useKeepalive } from "@/hooks/useKeepalive";
import { useSyncStore } from "@/stores/syncStore";
import { AutoSignIn } from "@/components/settings/AutoSignIn";
import { Section } from "./section";

const AUTH_TEXT: Record<AuthStatus, string> = {
  connected: "Active",
  pending: "Signing in…",
  disconnected: "Offline",
  expired: "Session expired",
};

export default function SettingsCanvasPage() {
  const {
    status: authStatus,
    connect: connectCanvas,
    disconnect: disconnectCanvas,
  } = useAuth();
  const scraping = useSyncStore((s) => s.scraping);
  const { status: ka, busy: kaBusy, error: kaError, toggle: kaToggle } = useKeepalive();

  return (
    <Section title="Canvas" description="canvas.lms.unimelb.edu.au">
      <div>
        <div className="flex items-center justify-between gap-4 py-2">
          <div className="flex items-center gap-2">
            <span
              className={cn(
                "w-1.5 h-1.5 rounded-full",
                authStatus === "connected"
                  ? "bg-success"
                  : authStatus === "pending"
                    ? "bg-warning animate-pulse"
                    : authStatus === "expired"
                      ? "bg-destructive"
                      : "bg-muted-foreground/40",
              )}
            />
            <span className="text-xs text-foreground">
              {AUTH_TEXT[authStatus]}
            </span>
            {authStatus === "pending" && (
              <span className="text-xs text-muted-foreground">
                — complete sign-in in the Canvas window
              </span>
            )}
            {authStatus === "expired" && (
              <span className="text-xs text-muted-foreground">
                — Canvas rejected the saved session, re-authenticate
              </span>
            )}
          </div>
          <div className="flex items-center gap-1.5">
            <Button
              variant={authStatus === "connected" ? "ghost" : "default"}
              size="xs"
              onClick={connectCanvas}
              disabled={authStatus === "pending"}
            >
              {authStatus === "pending" ? (
                <><CircleNotch size={13} className="animate-spin" /> Opening…</>
              ) : authStatus === "connected" || authStatus === "expired" ? (
                <><SignIn size={13} /> Re-authenticate</>
              ) : (
                <><SignIn size={13} /> Connect to Canvas</>
              )}
            </Button>
            {authStatus === "connected" && (
              <Button
                variant="ghost"
                size="xs"
                className="text-destructive hover:text-destructive hover:bg-destructive/10"
                onClick={disconnectCanvas}
                disabled={scraping}
                title={scraping ? "Cannot disconnect while syncing — cancel first" : undefined}
              >
                <X size={13} /> Disconnect
              </Button>
            )}
          </div>
        </div>

        <AutoSignIn />

        {ka?.supported && (
          <div className="flex items-center justify-between gap-3 py-2">
            <Label
              htmlFor="keepalive"
              className="text-xs font-normal text-foreground flex items-center gap-1.5"
            >
              Auto-refresh session
              <Tooltip>
                <TooltipTrigger asChild>
                  <Info size={12} className="text-muted-foreground/60" />
                </TooltipTrigger>
                <TooltipContent className="max-w-[220px]">
                  Refreshes the Canvas session every {ka.interval_hours}h in the
                  background so it doesn't expire while Oculus is closed, and
                  signs back in automatically if it has already lapsed.
                  {ka.enabled && ka.last_run ? ` Last run: ${ka.last_run}` : ""}
                </TooltipContent>
              </Tooltip>
              {kaBusy && <CircleNotch size={11} className="animate-spin" />}
            </Label>
            <Switch
              id="keepalive"
              checked={ka.enabled}
              disabled={kaBusy || authStatus !== "connected"}
              onCheckedChange={(checked) => kaToggle(checked, ka.interval_hours)}
              className="shrink-0"
            />
          </div>
        )}
        {kaError && (
          <p className="text-xs text-destructive py-2">{kaError}</p>
        )}
      </div>
    </Section>
  );
}
