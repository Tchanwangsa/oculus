import { pauseLecturePlayback } from "@/lib/lectures/playback";
import { playbackOwner, subscribePlaybackOwner } from "@/lib/lectures/playbackOwner";
import {
  isLibraryVideoPlaying,
  pauseLibraryVideos,
  subscribeLibraryVideos,
} from "@/lib/lectures/media/library";
import { music } from "@/lib/music";
import { toSilence, type Sounds } from "./rule";

/**
 * Wires `toSilence` (`./rule.ts`) to music's snapshots, the lecture playback
 * owner and library videos. Music counts as started however it started —
 * here or from the Spotify app on a phone.
 */

export interface MusicSource {
  playing: () => boolean;
  /** Called on every snapshot. */
  subscribe: (fn: () => void) => () => void;
}

/** Wires the rule once for the app; returns the unwiring. */
export function wireExclusivePlayback(source: MusicSource): () => void {
  const read = (): Sounds => ({
    music: source.playing(),
    local: playbackOwner().playing || isLibraryVideoPlaying(),
  });
  let prev = read();
  const check = () => {
    const next = read();
    const side = toSilence(prev, next);
    prev = next;
    if (side === "music") void music.pause().catch(() => {});
    if (side === "local") {
      pauseLecturePlayback();
      pauseLibraryVideos();
    }
  };
  const offs = [
    source.subscribe(check),
    subscribePlaybackOwner(check),
    subscribeLibraryVideos(check),
  ];
  return () => offs.forEach((off) => off());
}
