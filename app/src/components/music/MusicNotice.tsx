import { CircleNotch } from "@phosphor-icons/react";
import type { MusicSnapshot } from "@/lib/music";

/** The player's one status line: the error when there is one, else
 *  "Reconnecting…" while a dropped connection is made again (the track
 *  stays on show meanwhile). Nothing otherwise. */
export function MusicNotice({
  snapshot,
  error,
}: {
  snapshot: MusicSnapshot;
  error: string | null;
}) {
  if (error) return <p className="text-xs text-destructive">{error}</p>;
  if (!snapshot.reconnecting) return null;
  return (
    <p className="flex items-center gap-1.5 text-xs text-muted-foreground">
      <CircleNotch size={12} className="animate-spin" />
      Reconnecting…
    </p>
  );
}

/** The title without a track while a session is being made. */
export function connectingTitle(snapshot: MusicSnapshot): string | null {
  if (snapshot.reconnecting) return "Reconnecting…";
  return snapshot.connecting ? "Connecting…" : null;
}
