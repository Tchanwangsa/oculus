import { SpeakerHigh, SpeakerLow, SpeakerNone } from "@phosphor-icons/react";
import { Slider } from "@/components/ui/slider";
import { music, type MusicSnapshot } from "@/lib/music";
import { cn } from "@/lib/utils";
import { useSliderDraft } from "@/hooks/music/useSliderDraft";
import { useMusicStore } from "@/stores/music/musicStore";
import { MUSIC_SLIDER } from "./sliderStyle";

/** Spotify's volume for this device, 0–100, sent as it is dragged. */
export function MusicVolume({ snapshot }: { snapshot: MusicSnapshot }) {
  const run = useMusicStore((s) => s.run);
  const { draft, change, commit } = useSliderDraft(snapshot.volume);
  const volume = draft ?? snapshot.volume;
  const Icon = volume === 0 ? SpeakerNone : volume < 50 ? SpeakerLow : SpeakerHigh;

  return (
    <div className="flex items-center gap-2 text-muted-foreground">
      <Icon size={15} className="shrink-0" />
      <Slider
        min={0}
        max={100}
        step={1}
        value={[volume]}
        disabled={!snapshot.connected}
        onValueChange={([v]) => {
          change(v);
          run(() => music.setVolume(v));
        }}
        onValueCommit={([v]) => commit(v)}
        aria-label="Volume"
        className={cn("cursor-pointer py-1", MUSIC_SLIDER)}
      />
    </div>
  );
}
