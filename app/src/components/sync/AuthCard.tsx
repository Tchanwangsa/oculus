import { ArrowPathIcon } from "@heroicons/react/16/solid";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import type { AuthStatus } from "@/hooks/useAuth";
import { useKeepalive } from "@/hooks/useKeepalive";

interface AuthCardProps {
  status: AuthStatus;
  onConnect: () => void;
  onDisconnect: () => void;
  scraping?: boolean;
  children?: React.ReactNode;
}

export function AuthCard({ status, onConnect, onDisconnect, scraping = false, children }: AuthCardProps) {
  const { status: ka, busy: kaBusy, error: kaError, toggle: kaToggle } = useKeepalive();

  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <div className="flex items-center justify-between mb-0.5">
        <span className="font-medium text-[13px] text-foreground">Canvas</span>
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <span
            className={cn(
              "w-1.5 h-1.5 rounded-full",
              status === "connected"
                ? "bg-success"
                : status === "pending"
                  ? "bg-warning"
                  : "bg-muted-foreground/40",
            )}
          />
          {status === "connected"
            ? "Session active"
            : status === "pending"
              ? "Signing in…"
              : "Not connected"}
        </span>
      </div>
      <p className="text-xs text-muted-foreground mb-4">
        canvas.lms.unimelb.edu.au
      </p>

      <div className="flex gap-2">
        <Button
          variant={status === "connected" ? "outline" : "default"}
          size="sm"
          className="flex-1 h-8"
          onClick={onConnect}
          disabled={status === "pending"}
        >
          {status === "pending" ? (
            <><ArrowPathIcon className="size-[13px] animate-spin" /> Opening Canvas login…</>
          ) : status === "connected" ? (
            "Re-authenticate"
          ) : (
            "Connect to Canvas"
          )}
        </Button>
        {status === "connected" && (
          <Button
            variant="ghost"
            size="sm"
            className="h-8 text-destructive hover:text-destructive hover:bg-destructive/10"
            onClick={onDisconnect}
            disabled={scraping}
            title={scraping ? "Cannot disconnect while syncing — cancel first" : undefined}
          >
            Disconnect
          </Button>
        )}
      </div>

      {status === "pending" && (
        <p className="text-xs text-muted-foreground mt-3 text-center">
          Complete sign-in in the Canvas window, then return here.
        </p>
      )}

      {/* ── Background keep-alive ─────────────────────────────────────────── */}
      {ka?.supported && (
        <>
          <Separator className="my-4" />
          <label className="flex items-start gap-2.5 cursor-pointer group">
            <input
              type="checkbox"
              checked={ka.enabled}
              disabled={kaBusy || status !== "connected"}
              onChange={(e) => kaToggle(e.target.checked, ka.interval_hours)}
              className="mt-0.5 accent-primary cursor-pointer disabled:cursor-not-allowed"
            />
            <span className="flex-1 min-w-0">
              <span className="flex items-center gap-1.5 text-xs text-foreground">
                {kaBusy ? (
                  <ArrowPathIcon className="size-[11px] animate-spin" />
                ) : (
                  <ArrowPathIcon className={cn("size-[11px]", ka.enabled ? "text-success" : "text-muted-foreground")} />
                )}
                Keep session alive in the background
              </span>
              <span className="block text-xs text-muted-foreground mt-0.5">
                {status !== "connected"
                  ? "Connect to Canvas first."
                  : ka.enabled
                    ? `A background job refreshes the session every ${ka.interval_hours}h, even with Oculus closed.`
                    : `Refreshes the session every ${ka.interval_hours}h so it doesn't expire while Oculus is closed.`}
              </span>
              {ka.enabled && ka.last_run && (
                <span className="block text-xs text-muted-foreground/70 mt-0.5 font-mono truncate">
                  {ka.last_run}
                </span>
              )}
            </span>
          </label>
          {kaError && <p className="text-xs text-destructive mt-2">{kaError}</p>}
        </>
      )}

      {children}
    </div>
  );
}
