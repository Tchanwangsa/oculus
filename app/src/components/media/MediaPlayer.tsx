/**
 * The app's one media player: the clock, the control bar over the frame,
 * fullscreen, keys, captions and the dock's frame. Callers own the element and
 * what the dock shows — a lecture adopts the shared elements
 * (`lib/lectures/playback/`) and docks chapters and chat beside its
 * transcript; a library video renders its own `<video>`.
 *
 * `useMediaPlayer` holds the state, so a caller can derive from the playhead
 * (the playing chapter) before rendering `MediaPlayer` with it.
 */
import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  ArrowsIn,
  ArrowsOut,
  ClosedCaptioning,
  Pause,
  Play,
  SidebarSimple,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { fmtClockSecs } from "@/lib/lectures/media";
import { usePlayerPrefs } from "@/stores/lectures/playerPrefsStore";
import { CaptionOverlay } from "@/components/media/CaptionOverlay";
import { SpeedControl } from "@/components/media/SpeedControl";
import { VolumeControl } from "@/components/media/VolumeControl";
import { DockDropPreview, DockResizeHandle } from "@/components/media/MediaDock";
import { ControlButton } from "@/components/media/player/ControlButton";
import { DOCK_ICON_FACING, NO_STARTS } from "@/components/media/player/constants";
import { SeekBar } from "@/components/media/player/SeekBar";
import type { MediaPlayerState } from "@/components/media/player/useMediaPlayer";

export { ControlButton } from "@/components/media/player/ControlButton";
export {
  useMediaPlayer,
  type MediaPlayerOptions,
  type MediaPlayerState,
} from "@/components/media/player/useMediaPlayer";

interface MediaPlayerProps {
  player: MediaPlayerState;
  /** The picture: frames an element is moved into, a `<video>` of the
   *  caller's own, or what shows before there is one. */
  children: ReactNode;
  /** Over the picture and the (then inert) control bar — a lecture's
   *  "Playing in the other pane" still. */
  overlay?: ReactNode;
  /** The dock panel, usually a `MediaDock`. */
  dock: ReactNode;
  /** Can play: a source is loaded. */
  canPlay: boolean;
  /** Streams the scrub bar's hover thumbnail. */
  previewSrc: string | null;
  /** Chapter boundaries notched into the scrub bar. */
  chapters?: number[];
  /** Second the lecture's content ends, flagged on the scrub bar. */
  endAt?: number | null;
  /** Above the scrub bar — the lecture's chapter name. */
  aboveSeek?: ReactNode;
  /** Buttons between speed and the dock button. */
  controls?: ReactNode;
  /** The dock button's label. */
  dockLabel: string;
  /** The caller's error, shown in place of the element's. */
  error?: string | null;
}

/** The player's frame: picture, captions, control bar, dock. */
export function MediaPlayer({
  player,
  children,
  overlay,
  dock: dockPanel,
  canPlay,
  previewSrc,
  chapters = NO_STARTS,
  endAt = null,
  aboveSeek,
  controls,
  dockLabel,
  error,
}: MediaPlayerProps) {
  const {
    cues,
    activeCueIdx,
    isPlaying,
    currentTime,
    duration,
    isFullscreen,
    controlsVisible,
    elsewhere,
    hasCaptions,
    captionsEnabled,
    dockOpen,
    dock: { dock, height, width, dropTarget, startResize },
    containerRef,
    videoAreaRef,
    holds,
  } = player;
  const volume = usePlayerPrefs((s) => s.volume);
  const muted = usePlayerPrefs((s) => s.muted);
  const speed = usePlayerPrefs((s) => s.speed);
  const setPrefs = usePlayerPrefs((s) => s.set);
  const shownError = error ?? player.error;

  // The bar's real height (the chapter title above the seek bar makes it taller
  // than its buttons), so the caption rides exactly clear of it.
  const barRef = useRef<HTMLDivElement>(null);
  const [barHeight, setBarHeight] = useState(0);
  useEffect(() => {
    const el = barRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBarHeight(el.offsetHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  return (
    // The dock side is a flex direction: `*-reverse` puts the panel before the
    // video stack visually while leaving the divider between the two.
    <div
      ref={containerRef}
      className={cn(
        "flex overflow-hidden min-h-0 min-w-0 bg-background relative",
        // Over the sidebar and the tab strip alike.
        isFullscreen ? "fixed inset-0 z-50" : "flex-1",
        dock === "bottom" && "flex-col",
        dock === "top" && "flex-col-reverse",
        dock === "right" && "flex-row",
        dock === "left" && "flex-row-reverse",
      )}
    >
      <div className="flex-1 flex flex-col min-h-0 min-w-0 overflow-hidden">
        {/* Video area — the controls live over the frame, YouTube-style */}
        <div
          ref={videoAreaRef}
          className={cn(
            "flex-1 bg-black flex flex-col min-h-0 relative overflow-hidden",
            !controlsVisible && !elsewhere && "cursor-none",
          )}
          onPointerMove={player.revealControls}
          onPointerLeave={holds.leaveArea}
        >
          {children}
          {overlay}
          {captionsEnabled && activeCueIdx >= 0 && !elsewhere && (
            <CaptionOverlay
              text={cues[activeCueIdx]?.text ?? ""}
              boundsRef={videoAreaRef}
              // Lift clear of the control bar while it shows.
              lift={controlsVisible ? barHeight : 0}
            />
          )}

          {/* Controls — one scrim, the scrub bar across the top of it */}
          <div
            ref={barRef}
            className={cn(
              "absolute inset-x-0 bottom-0 z-30 transition-opacity will-change-[opacity] duration-200",
              controlsVisible ? "opacity-100" : "opacity-0 pointer-events-none",
            )}
            inert={elsewhere}
            onPointerEnter={holds.enterBar}
            onPointerLeave={holds.leaveBar}
          >
            <div className="pointer-events-none absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-black/85 via-black/45 to-transparent" />

            <div className="relative px-3 pb-1">
              {aboveSeek}

              <SeekBar
                value={Math.min(currentTime, duration)}
                max={duration || 1}
                previewSrc={previewSrc}
                chapters={chapters}
                endAt={endAt}
                onSeek={player.scrub}
              />

              <div className="flex h-9 items-center gap-0.5">
                <ControlButton
                  label={isPlaying ? "Pause" : "Play"}
                  onClick={player.togglePlay}
                  disabled={!canPlay}
                >
                  {isPlaying ? (
                    <Pause size={16} weight="fill" />
                  ) : (
                    <Play size={16} weight="fill" />
                  )}
                </ControlButton>

                <VolumeControl
                  volume={volume}
                  muted={muted}
                  onChange={(v) =>
                    // Dragging up from silence is a request to hear it.
                    setPrefs({ volume: v, muted: muted && v === 0 })
                  }
                  onToggleMute={() => setPrefs({ muted: !muted })}
                  onDraggingChange={holds.volumeDragging}
                />

                <span className="ml-1 select-none whitespace-nowrap text-[11.5px] tabular-nums text-white/85">
                  {fmtClockSecs(Math.floor(currentTime), duration >= 3600)}{" "}
                  <span className="text-white/45">/</span>{" "}
                  {fmtClockSecs(duration)}
                </span>

                <div className="flex-1" />

                <SpeedControl
                  speed={speed}
                  onChange={(s) => setPrefs({ speed: s })}
                  onOpenChange={player.onPanelOpenChange}
                />

                {controls}

                {/* Labelled by the tab in front: it folds the whole dock. */}
                <ControlButton
                  label={dockLabel}
                  active={dockOpen}
                  onClick={player.toggleDock}
                >
                  <SidebarSimple
                    size={16}
                    className={cn("transition-transform", DOCK_ICON_FACING[dock])}
                  />
                </ControlButton>

                <ControlButton
                  label={
                    captionsEnabled ? "Hide captions (C)" : "Show captions (C)"
                  }
                  active={captionsEnabled}
                  disabled={!hasCaptions}
                  onClick={player.toggleCaptions}
                >
                  <ClosedCaptioning
                    size={17}
                    weight={captionsEnabled ? "fill" : "regular"}
                  />
                </ControlButton>

                <ControlButton
                  label={isFullscreen ? "Exit fullscreen (F)" : "Fullscreen (F)"}
                  onClick={player.toggleFullscreen}
                >
                  {isFullscreen ? <ArrowsIn size={16} /> : <ArrowsOut size={16} />}
                </ControlButton>
              </div>
            </div>
          </div>
        </div>

        {shownError && (
          <div className="shrink-0 px-3 py-1.5 text-[11px] text-destructive border-t border-border bg-destructive/5 break-words">
            {shownError}
          </div>
        )}
      </div>

      {dockOpen && <DockResizeHandle dock={dock} onPointerDown={startResize} />}
      {dockPanel}

      {dropTarget && (
        <DockDropPreview dock={dropTarget} height={height} width={width} />
      )}
    </div>
  );
}
