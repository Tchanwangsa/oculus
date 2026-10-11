import { SpeakerHigh, SpeakerLow, SpeakerNone } from "@phosphor-icons/react";
import { Slider } from "@/components/ui/slider";
import { music, shownPlayback, type MusicSnapshot } from "@/lib/music";
import { cn } from "@/lib/utils";
import { useSliderDraft } from "@/hooks/music/useSliderDraft";
import { useMusicStore } from "@/stores/music/musicStore";
import { MUSIC_SLIDER } from "./sliderStyle";

/** Spotify's volume, 0–100, for the device the player shows: sent as it is
 *  dragged for this device, on release for another one, where each change
 *  is a request to Spotify. Disabled for a device that takes no volume. */
export function MusicVolume({ snapshot }: { snapshot: MusicSnapshot }) {
  const run = useMusicStore((s) => s.run);
  const playback = shownPlayback(snapshot);
  const remote = playback.device !== null;
  const { draft, change, commit } = useSliderDraft(playback.volume);
  const volume = draft ?? playback.volume ?? 0;
  const Icon = volume === 0 ? SpeakerNone : volume < 50 ? SpeakerLow : SpeakerHigh;

  return (
    <div className="flex items-center gap-2 text-muted-foreground">
      <Icon size={15} className="shrink-0" />
      <Slider
        min={0}
        max={100}
        step={1}
        value={[volume]}
        disabled={!snapshot.connected || playback.volume === null}
        onValueChange={([v]) => {
          change(v);
          if (!remote) run(() => music.setVolume(v));
        }}
        onValueCommit={([v]) => {
          commit(v);
          if (remote) run(() => music.setVolume(v));
        }}
        aria-label="Volume"
        className={cn("cursor-pointer py-1", MUSIC_SLIDER)}
      />
    </div>
  );
}
