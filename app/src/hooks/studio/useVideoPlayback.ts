import { useEffect, useMemo, useState } from "react";

import { useMediaPlayer } from "@/components/media/MediaPlayer";
import { useLibraryPlayback } from "@/components/media/player/useLibraryPlayback";
import { useTabActive } from "@/components/tabs/TabContext";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { mediaSrc, parseVtt, spanAt, type Cue } from "@/lib/lectures/media";
import { playableRenders } from "@/lib/studio/exports";
import type { RenderPreset } from "@/lib/studio/render";
import { renderCaptions, type VideoDetail } from "@/lib/studio/videos";
import type { Dock } from "@/stores/lectures/playerPrefsStore";

/** Without captions beside it, a render has no cues. */
const NO_CUES: Cue[] = [];
const noop = () => {};
/** The picture keeps this much width beside a side dock; a narrower page
 *  docks at the bottom instead. */
const DOCK_KEEP_WIDTH = 480;

const DOCK_SIDE_KEY = "oculus-studio-video-dock-side";
const SIDES: readonly Dock[] = ["right", "left", "top", "bottom"];

/** A stored dock side; a video dock starts on the right. */
const readDockSide = (raw: string | null): Dock => SIDES.find((d) => d === raw) ?? "right";

/** The scene starts the scrub bar notches and the strip follows. */
export function sceneStarts(detail: VideoDetail): number[] {
  return detail.scenes.map((s) => s.start ?? NaN);
}

/**
 * A video's render in the media player: the newest video render (the reader
 * can pick another), its URL, the `<video>` element, and the scene under the
 * playhead. A re-render of the same file starts the player over. The
 * render's `.vtt` beside it, when there is one, is the CC. `docked` is
 * whether the page gives the player a dock, which T then folds.
 */
export function useVideoPlayback(detail: VideoDetail, docked: boolean) {
  const [picked, setPicked] = useState<RenderPreset | null>(null);
  const playable = useMemo(() => playableRenders(detail.renders), [detail.renders]);
  const shown = useMemo(
    () => playable.find((r) => r.preset === picked) ?? playable[0] ?? null,
    [playable, picked],
  );

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

  const [cues, setCues] = useState<Cue[]>(NO_CUES);
  const slug = detail.row.slug;
  // Captions drawn into the picture need no CC button; otherwise the .vtt
  // beside the render is the CC.
  const sidecar = !detail.captions && shown?.captions ? shown.preset : null;
  useEffect(() => {
    setCues(NO_CUES);
    if (!sidecar) return;
    let stale = false;
    renderCaptions(slug, sidecar)
      .then((vtt) => !stale && setCues(parseVtt(vtt)))
      .catch((e) => console.error("render captions failed", e));
    return () => {
      stale = true;
    };
  }, [slug, sidecar, version]);

  // A video dock keeps its own side (default right), so the lecture dock keeps
  // its own too.
  const [side, setSide] = useStoredState(DOCK_SIDE_KEY, readDockSide);
  const dockSide = useMemo(() => ({ value: side, set: setSide }), [side, setSide]);

  const player = useMediaPlayer({
    cues,
    resetKey: `${path}:${version}`,
    fallbackDuration: duration,
    dockTab: "transcript",
    dockKeepWidth: DOCK_KEEP_WIDTH,
    dockSide: docked ? dockSide : undefined,
    hasCaptions: cues.length > 0,
    // Without a dock (the thread page) T has nothing to show.
    onToggleDock: docked ? undefined : noop,
  });

  const [el, setEl] = useState<HTMLVideoElement | null>(null);
  const { attach } = player;
  useEffect(() => attach(el), [el, attach]);
  useLibraryPlayback(el, useTabActive());

  const starts = useMemo(() => sceneStarts(detail), [detail]);
  // Scenes with no start (not narrated) never light.
  const known = useMemo(() => starts.filter((s) => !Number.isNaN(s)), [starts]);
  const current = known.length === starts.length ? spanAt(known, player.currentTime) : -1;

  return {
    player,
    shown,
    playable,
    src,
    setEl,
    pick: setPicked,
    current,
    chapters: known.filter((s) => s > 0),
    hasCaptions: cues.length > 0,
  };
}

export type VideoPlayback = ReturnType<typeof useVideoPlayback>;
