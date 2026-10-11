import { useEffect, useState } from "react";
import { Section } from "@/components/settings/shared/section";
import { Switch } from "@/components/ui/switch";
import { useMusicPrefsStore } from "@/stores/music/musicPrefsStore";
import { useMusicStore } from "@/stores/music/musicStore";
import { MusicStatus } from "./MusicStatus";

/**
 * Settings → Music: the opt-in switch (the `music` settings row, which also
 * decides whether Oculus connects at startup) and, while on, the session.
 */
export function MusicSection() {
  const { enabled, loaded, load, setEnabled } = useMusicPrefsStore();
  const snapshot = useMusicStore((s) => s.snapshot);
  const [failure, setFailure] = useState<string | null>(null);

  // Also loaded by `useMusicState` at startup; re-read in case this renders first.
  useEffect(() => {
    if (!loaded) void load().catch(() => {});
  }, [loaded, load]);

  const toggle = (on: boolean) => {
    setFailure(null);
    setEnabled(on).catch((e: unknown) => setFailure(String(e)));
  };

  return (
    <Section title="Music" description="Spotify, played in Oculus.">
      <div className="flex items-start justify-between gap-6 py-2.5">
        <label htmlFor="music-enabled" className="min-w-0">
          <span className="text-xs text-foreground">Play Spotify in Oculus</span>
          <p className="mt-0.5 text-[11px] leading-relaxed text-muted-foreground">
            Needs Spotify Premium. Oculus shows up in Spotify as a device called
            Oculus and plays through librespot, an unofficial Spotify client, so
            use it at your own risk.
          </p>
        </label>
        <Switch
          id="music-enabled"
          checked={enabled}
          disabled={!loaded}
          onCheckedChange={toggle}
          className="shrink-0"
        />
      </div>
      {failure && <p className="py-1 text-xs text-destructive">{failure}</p>}
      {enabled && snapshot && <MusicStatus snapshot={snapshot} />}
    </Section>
  );
}
