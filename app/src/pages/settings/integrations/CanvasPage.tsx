import { CircleNotch, SignIn, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useAuth, type AuthStatus } from "@/hooks/sync/useAuth";
import { useSyncStore } from "@/stores/sync/syncStore";
import { AutoSignIn } from "@/components/settings/web/AutoSignIn";
import { Section } from "@/components/settings/shared/section";

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
      </div>
    </Section>
  );
}
