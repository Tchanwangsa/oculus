import { useEffect, useMemo, useState } from "react";

import { useMediaPlayer } from "@/components/media/MediaPlayer";
import { useLibraryPlayback } from "@/components/media/player/useLibraryPlayback";
import { useTabActive } from "@/components/tabs/TabContext";
import { mediaSrc, spanAt, type Cue } from "@/lib/lectures/media";
import type { RenderFile, VideoDetail } from "@/lib/studio/videos";

/** A render has no transcript, so the player gets no cues. */
const NO_CUES: Cue[] = [];
const noop = () => {};

/** The scene starts the scrub bar notches and the strip follows. */
export function sceneStarts(detail: VideoDetail): number[] {
  return detail.scenes.map((s) => s.start ?? NaN);
}

/**
 * A video's render in the media player: the newest draft or final (the
 * reader can pick the other), its URL, the `<video>` element, and the scene
 * under the playhead. A re-render of the same file starts the player over.
 */
export function useVideoPlayback(detail: VideoDetail) {
  const [picked, setPicked] = useState<RenderFile["quality"] | null>(null);
  const shown = useMemo(() => {
    const byQuality = detail.renders.find((r) => r.quality === picked);
    return byQuality ?? detail.renders.slice().sort((a, b) => b.modified - a.modified)[0] ?? null;
  }, [detail.renders, picked]);

  const [src, setSrc] = useState<string | null>(null);
  const path = shown?.path;
  const version = shown?.modified;
  useEffect(() => {
    setSrc(null);
    if (!path) return;
    let stale = false;
    mediaSrc(path).then((url) => !stale && setSrc(`${url}&v=${version}`));
    return () => {
      stale = true;
    };
  }, [path, version]);

  const duration = useMemo(() => {
    const last = detail.scenes[detail.scenes.length - 1];
    return last?.start != null && last.duration != null ? last.start + last.duration : 0;
  }, [detail.scenes]);

  const player = useMediaPlayer({
    cues: NO_CUES,
    resetKey: `${path}:${version}`,
    fallbackDuration: duration,
    dockTab: "transcript",
    hasCaptions: false,
    // T has no dock to show here.
    onToggleDock: noop,
  });

  const [el, setEl] = useState<HTMLVideoElement | null>(null);
  const { attach } = player;
  useEffect(() => attach(el), [el, attach]);
  useLibraryPlayback(el, useTabActive());

  const starts = useMemo(() => sceneStarts(detail), [detail]);
  // Scenes with no start (not narrated) never light.
  const known = useMemo(() => starts.filter((s) => !Number.isNaN(s)), [starts]);
  const current = known.length === starts.length ? spanAt(known, player.currentTime) : -1;

  return { player, shown, src, setEl, pick: setPicked, current, chapters: known.filter((s) => s > 0) };
}

export type VideoPlayback = ReturnType<typeof useVideoPlayback>;
