import {
  ShieldCheck,
  XCircle,
  Loader2,
  Globe,
  Clock,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import type { AuthStatus } from "@/hooks/useAuth";

const AUTH_BADGE: Record<AuthStatus, { label: string; variant: "success" | "secondary" | "warning" }> = {
  connected:    { label: "Connected",    variant: "success"   },
  disconnected: { label: "Disconnected", variant: "secondary" },
  pending:      { label: "Signing in…",  variant: "warning"   },
};

interface AuthCardProps {
  status: AuthStatus;
  onConnect: () => void;
  onDisconnect: () => void;
  scraping?: boolean;
  children?: React.ReactNode;
}

export function AuthCard({ status, onConnect, onDisconnect, scraping = false, children }: AuthCardProps) {
  const badge = AUTH_BADGE[status];

  return (
    <div className="rounded-xl border border-border bg-card p-5">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          {status === "connected" ? (
            <ShieldCheck size={16} className="text-success" />
          ) : status === "pending" ? (
            <Loader2 size={16} className="text-primary animate-spin" />
          ) : (
            <XCircle size={16} className="text-muted-foreground" />
          )}
          <span className="font-semibold text-sm text-foreground">Canvas Connection</span>
        </div>
        <Badge variant={badge.variant}>{badge.label}</Badge>
      </div>

      <div className="flex items-center gap-3 text-xs text-muted-foreground mb-4">
        <Globe size={13} />
        <span>canvas.lms.unimelb.edu.au</span>
        {status === "connected" && (
          <span className="ml-auto flex items-center gap-1">
            <Clock size={11} /> Session active
          </span>
        )}
      </div>

      <Separator className="mb-4" />

      <div className="flex gap-2">
        <Button
          variant={status === "connected" ? "outline" : "default"}
          size="sm"
          className="flex-1"
          onClick={onConnect}
          disabled={status === "pending"}
        >
          {status === "pending" ? (
            <><Loader2 size={13} className="animate-spin" /> Opening Canvas login…</>
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
            className="text-destructive hover:text-destructive hover:bg-destructive/10"
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

      {children}
    </div>
  );
}
