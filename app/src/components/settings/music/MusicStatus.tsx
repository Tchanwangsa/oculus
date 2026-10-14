import { useState } from "react";
import { CircleNotch, SignIn, SignOut, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { StatRow } from "@/components/settings/shared/section";
import { music, type MusicSnapshot } from "@/lib/music";
import { cn } from "@/lib/utils";

type Status = "connected" | "reconnecting" | "connecting" | "signed-out" | "failed" | "idle";

function statusOf(s: MusicSnapshot): Status {
  if (s.reconnecting) return "reconnecting";
  if (s.connecting) return "connecting";
  if (s.connected) return "connected";
  if (s.needs_login) return "signed-out";
  return s.error ? "failed" : "idle";
}

const DOT: Record<Status, string> = {
  connected: "bg-success",
  reconnecting: "bg-warning animate-pulse",
  connecting: "bg-warning animate-pulse",
  "signed-out": "bg-muted-foreground/40",
  failed: "bg-destructive",
  idle: "bg-muted-foreground/40",
};

/**
 * The session's status, the device name and its buttons, shown while
 * Settings → Music is on. Every button is a login or a connection: nothing
 * here plays (docs/harness.md: Settings makes no side-effect calls).
 */
export function MusicStatus({ snapshot }: { snapshot: MusicSnapshot }) {
  const [failure, setFailure] = useState<string | null>(null);
  const status = statusOf(snapshot);
  const text: Record<Status, string> = {
    connected: `Connected as ${snapshot.account ?? "your account"}`,
    reconnecting: "Reconnecting…",
    connecting: "Connecting…",
    "signed-out": "Not signed in",
    failed: "Stopped by the error below",
    idle: "Not connected",
  };

  const act = (command: () => Promise<unknown>) => {
    setFailure(null);
    command().catch((e: unknown) => setFailure(String(e)));
  };
  const error = snapshot.error ?? failure;

  return (
    <div>
      <div className="settings-row flex items-center justify-between gap-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className={cn("size-1.5 shrink-0 rounded-full", DOT[status])} />
          <span className="truncate text-xs text-foreground">{text[status]}</span>
        </div>
        <div className="settings-actions flex shrink-0 items-center gap-1.5">
          {snapshot.needs_login ? (
            <Button size="xs" onClick={() => act(music.login)}>
              <SignIn size={13} /> Log in with Spotify
            </Button>
          ) : snapshot.connected ? (
            <Button variant="ghost" size="xs" onClick={() => act(music.disconnect)}>
              <X size={13} /> Disconnect
            </Button>
          ) : (
            <Button
              size="xs"
              disabled={snapshot.connecting}
              onClick={() => act(music.connect)}
            >
              {snapshot.connecting ? (
                <><CircleNotch size={13} className="animate-spin" /> Connecting…</>
              ) : (
                <><SignIn size={13} /> {status === "failed" ? "Try again" : "Connect"}</>
              )}
            </Button>
          )}
          {!snapshot.needs_login && (
            <Button
              variant="ghost"
              size="xs"
              className="text-destructive hover:bg-destructive/10 hover:text-destructive"
              onClick={() => act(music.signOut)}
            >
              <SignOut size={13} /> Sign out
            </Button>
          )}
        </div>
      </div>
      <StatRow label="Device" value={snapshot.device_name} />
      {error && <p className="py-1 text-xs text-destructive">{error}</p>}
    </div>
  );
}
