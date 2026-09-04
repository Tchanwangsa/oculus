import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowsIn,
  ArrowsOut,
  CaretRight,
  CircleNotch,
  DownloadSimple,
  FileText,
  Pause,
  Play,
  SpeakerHigh,
  Subtitles,
  SubtitlesSlash,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import {
  downloadLecture,
  isDownloading,
  useLectureDownloads,
} from "@/stores/lectureDownloadStore";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { mediaSrc } from "@/lib/media";
import {
  updateLectureTranscriptPath,
  updateLectureProgress,
  markLectureComplete,
  type Lecture,
} from "@/lib/db";
import {
  parseVtt,
  fmtDuration,
  fmtTime,
  fmtLectureDate,
  type Cue,
} from "@/lib/lectures";

/**
 * Video scrub bar. Not the shadcn Slider: it mixes `clientX` with
 * `getBoundingClientRect`, which is one measurement more than this needs.
 * `offsetX / offsetWidth` stays entirely in the element's own coordinate
 * space, so a click lands exactly where the pointer is.
 */
function SeekBar({
  value,
  max,
  onSeek,
}: {
  value: number;
  max: number;
  onSeek: (seconds: number) => void;
}) {
  const pct = Math.min(100, Math.max(0, (value / max) * 100));

  const seekFromEvent = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    const frac = e.nativeEvent.offsetX / el.offsetWidth;
    onSeek(Math.min(1, Math.max(0, frac)) * max);
  };

  return (
    <div
      role="slider"
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={Math.floor(max)}
      aria-valuenow={Math.floor(value)}
      className="relative flex flex-1 items-center h-4 cursor-pointer touch-none select-none"
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        seekFromEvent(e);
      }}
      onPointerMove={(e) => {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) seekFromEvent(e);
      }}
    >
      {/* pointer-events-none children keep `offsetX` relative to the root */}
      <div className="pointer-events-none relative h-1.5 w-full overflow-hidden rounded-full bg-muted">
        <div className="absolute h-full bg-primary" style={{ width: `${pct}%` }} />
      </div>
      <div
        className="pointer-events-none absolute size-3 -translate-x-1/2 rounded-full border border-primary bg-white shadow-sm"
        style={{ left: `${pct}%` }}
      />
    </div>
  );
}

interface LecturePlayerProps {
  lecture: Lecture;
  /** Fired after anything persisted changes (progress, downloads). */
  onRefresh: () => void;
}

/**
 * The whole lecture player — video, controls bar, transcript. Self-contained:
 * loads its own transcript, tracks its own download, saves its own progress.
 * Wrapped by the lectures-tab peek and the standalone full-page view alike.
 */
export function LecturePlayer({ lecture, onRefresh }: LecturePlayerProps) {
  const [cues, setCues] = useState<Cue[]>([]);
  const [activeCueIdx, setActiveCueIdx] = useState(-1);
  const [isPlaying, setIsPlaying] = useState(false);
  const [speed, setSpeed] = useState(1.0);
  const [currentTime, setCurrentTime] = useState(0);
  const [transcriptVisible, setTranscriptVisible] = useState(true);
  const [captionsEnabled, setCaptionsEnabled] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Global on purpose: a download outlives this component (close the peek,
  // reopen it — the same download is still running in Rust).
  const downloads = useLectureDownloads();
  const downloading = isDownloading(downloads, lecture.id);
  const dlProgress = downloads.progress[lecture.id] ?? null;

  const videoRef = useRef<HTMLVideoElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const transcriptRef = useRef<HTMLDivElement>(null);
  const progressSaveRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const togglePlayRef = useRef<() => void>(() => {});

  // Served over localhost HTTP, not convertFileSrc — WebKit's media stack
  // refuses custom-scheme (asset://) sources outright. See lib/media.ts.
  const [videoSrc, setVideoSrc] = useState<string | null>(null);
  useEffect(() => {
    let stale = false;
    if (!lecture.video_path) {
      setVideoSrc(null);
      return;
    }
    mediaSrc(lecture.video_path).then((url) => {
      if (!stale) setVideoSrc(url);
    });
    return () => {
      stale = true;
    };
  }, [lecture.video_path]);

  // ── Transcript loading ───────────────────────────────────────────────────

  const loadTranscript = useCallback(async (path: string) => {
    try {
      const vtt = await invoke<string>("echo360_read_transcript", { path });
      setCues(parseVtt(vtt));
    } catch {
      /* transcript unreadable */
    }
  }, []);

  // Reset per lecture: fresh transcript, restored progress position.
  useEffect(() => {
    setCues([]);
    setActiveCueIdx(-1);
    setCurrentTime(0);
    setIsPlaying(false);
    setTranscriptVisible(true);
    setCaptionsEnabled(false);
    setError(null);

    if (lecture.transcript_path) loadTranscript(lecture.transcript_path);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lecture.id, lecture.transcript_path, lecture.video_path]);

  // Restore saved progress once the (async-resolved) source has metadata.
  const handleLoadedMetadata = () => {
    if (videoRef.current && lecture.progress_seconds > 5) {
      videoRef.current.currentTime = lecture.progress_seconds;
    }
  };

  // ── Fullscreen ───────────────────────────────────────────────────────────

  const handleToggleFullscreen = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    if (document.fullscreenElement) {
      document.exitFullscreen().catch(() => {});
    } else {
      el.requestFullscreen().catch(() => {});
    }
  }, []);

  useEffect(() => {
    const handler = () => setIsFullscreen(!!document.fullscreenElement);
    document.addEventListener("fullscreenchange", handler);
    return () => document.removeEventListener("fullscreenchange", handler);
  }, []);

  // ── Keyboard shortcuts ───────────────────────────────────────────────────

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      switch (e.key) {
        case " ":
          e.preventDefault();
          togglePlayRef.current();
          break;
        case "ArrowLeft":
          e.preventDefault();
          if (videoRef.current)
            videoRef.current.currentTime = Math.max(
              0,
              videoRef.current.currentTime - 5,
            );
          break;
        case "ArrowRight":
          e.preventDefault();
          if (videoRef.current) videoRef.current.currentTime += 5;
          break;
        case "f":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            handleToggleFullscreen();
          }
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleToggleFullscreen]);

  // ── Downloads ────────────────────────────────────────────────────────────

  // Downloads the transcript alongside the video; both land in the DB before
  // it resolves, so the refresh picks the paths up.
  const handleDownloadVideo = async () => {
    setError(null);
    try {
      await downloadLecture(lecture);
      onRefresh();
    } catch (e) {
      setError(`Download failed: ${e}`);
    }
  };

  const handleDownloadTranscript = async () => {
    setError(null);
    try {
      const path = await invoke<string>("echo360_download_transcript", {
        lessonId: lecture.lesson_id,
        mediaId: lecture.id,
        canvasCourseId: lecture.subject_id,
      });
      await updateLectureTranscriptPath(lecture.id, path);
      onRefresh();
      await loadTranscript(path);
      setTranscriptVisible(true);
    } catch (e) {
      setError(`Transcript download failed: ${e}`);
    }
  };

  // ── Playback ─────────────────────────────────────────────────────────────

  const handleTimeUpdate = () => {
    const v = videoRef.current;
    if (!v) return;
    const t = v.currentTime;
    setCurrentTime(t);

    let idx = -1;
    for (let i = cues.length - 1; i >= 0; i--) {
      if (t >= cues[i].start) {
        idx = i;
        break;
      }
    }
    setActiveCueIdx(idx);
    if (idx >= 0 && transcriptRef.current) {
      const el = transcriptRef.current.querySelector(`[data-cue="${idx}"]`);
      el?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }

    // Debounce progress save (save user-facing time)
    if (progressSaveRef.current) clearTimeout(progressSaveRef.current);
    progressSaveRef.current = setTimeout(async () => {
      await updateLectureProgress(lecture.id, Math.floor(v.currentTime));
      if (
        lecture.duration_seconds > 0 &&
        v.currentTime >= lecture.duration_seconds - 30
      ) {
        await markLectureComplete(lecture.id);
      }
      onRefresh();
    }, 5000);
  };

  const togglePlay = () => {
    const v = videoRef.current;
    if (!v) return;
    if (v.paused) {
      v.play();
      setIsPlaying(true);
    } else {
      v.pause();
      setIsPlaying(false);
    }
  };
  togglePlayRef.current = togglePlay;

  const handleSpeedChange = (s: number) => {
    setSpeed(s);
    if (videoRef.current) videoRef.current.playbackRate = s;
  };

  // ── Render ───────────────────────────────────────────────────────────────

  return (
    <div
      ref={containerRef}
      className="flex-1 flex flex-col overflow-hidden min-h-0 bg-background"
    >
      {/* Video area */}
      <div className="flex-1 bg-black flex flex-col min-h-0 relative overflow-hidden">
        {videoSrc ? (
          <video
            ref={videoRef}
            src={videoSrc}
            className="w-full h-full object-contain"
            onTimeUpdate={handleTimeUpdate}
            onLoadedMetadata={handleLoadedMetadata}
            onPlay={() => setIsPlaying(true)}
            onPause={() => setIsPlaying(false)}
            onEnded={() => setIsPlaying(false)}
            onError={() => {
              const err = videoRef.current?.error;
              setError(
                `Video failed to load (code ${err?.code ?? "?"}: ${
                  err?.message || "unknown"
                })`,
              );
            }}
          />
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center gap-4 text-white/60 p-6">
            <p className="text-sm font-medium text-white">{lecture.title}</p>
            <p className="text-xs">
              {fmtLectureDate(lecture.date)} · {fmtDuration(lecture.duration_seconds)}
            </p>
            {downloading ? (
              <div className="flex items-center gap-2 text-sm">
                <CircleNotch size={16} className="animate-spin" />
                <span>
                  {dlProgress?.phase === "trimming"
                    ? "Trimming…"
                    : `Downloading… ${dlProgress?.percent ?? 0}%`}
                </span>
              </div>
            ) : (
              <Button
                size="sm"
                className="gap-2 bg-white/10 hover:bg-white/20 text-white border-white/20"
                variant="outline"
                onClick={handleDownloadVideo}
              >
                <DownloadSimple size={14} /> Download video
              </Button>
            )}
          </div>
        )}
        {captionsEnabled && activeCueIdx >= 0 && (
          <div className="absolute bottom-8 left-0 right-0 flex justify-center pointer-events-none z-10">
            <span
              className="px-3 py-1.5 rounded text-sm text-white text-center max-w-[85%] leading-snug"
              style={{
                backgroundColor: "rgba(0,0,0,0.75)",
                textShadow: "0 1px 2px rgba(0,0,0,0.8)",
              }}
            >
              {cues[activeCueIdx]?.text}
            </span>
          </div>
        )}
      </div>

      {error && (
        <div className="shrink-0 px-3 py-1.5 text-[11px] text-destructive border-t border-border bg-destructive/5 break-words">
          {error}
        </div>
      )}

      {/* Controls bar */}
      <div className="h-10 shrink-0 bg-card border-t border-border flex items-center px-3 gap-3">
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={togglePlay}
          disabled={!videoSrc}
          aria-label={isPlaying ? "Pause" : "Play"}
        >
          {isPlaying ? <Pause size={14} /> : <Play size={14} />}
        </Button>

        <span className="text-[11px] text-muted-foreground font-mono whitespace-nowrap shrink-0">
          {fmtTime(Math.floor(currentTime), lecture.duration_seconds >= 3600)} /{" "}
          {fmtTime(lecture.duration_seconds)}
        </span>

        <SeekBar
          value={Math.min(currentTime, lecture.duration_seconds)}
          max={lecture.duration_seconds || 1}
          onSeek={(v) => {
            setCurrentTime(v);
            if (videoRef.current) videoRef.current.currentTime = v;
          }}
        />

        {/* Speed */}
        <div className="flex items-center gap-1">
          <SpeakerHigh size={11} className="text-muted-foreground" />
          <Select
            value={String(speed)}
            onValueChange={(v) => handleSpeedChange(Number(v))}
          >
            <SelectTrigger
              size="sm"
              className="h-7 border-0 bg-transparent px-1 text-[11px] shadow-none dark:bg-transparent dark:hover:bg-transparent"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent side="top">
              {[0.5, 0.75, 1, 1.25, 1.5, 1.75, 2].map((s) => (
                <SelectItem key={s} value={String(s)} className="text-xs">
                  {s}×
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* Action buttons */}
        <div className="flex items-center gap-1 ml-1">
          {!lecture.video_path && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={handleDownloadVideo}
                  disabled={downloading}
                  aria-label="Download video"
                  className="text-muted-foreground hover:text-foreground"
                >
                  {downloading ? (
                    <CircleNotch size={13} className="animate-spin" />
                  ) : (
                    <DownloadSimple size={13} />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>Download video</TooltipContent>
            </Tooltip>
          )}

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => {
                  if (lecture.transcript_path) {
                    if (cues.length > 0) {
                      setTranscriptVisible((v) => !v);
                    } else {
                      loadTranscript(lecture.transcript_path);
                    }
                  } else {
                    handleDownloadTranscript();
                  }
                }}
                aria-label={
                  lecture.transcript_path
                    ? transcriptVisible
                      ? "Hide transcript"
                      : "Show transcript"
                    : "Download transcript"
                }
                className={cn(
                  lecture.transcript_path && transcriptVisible
                    ? "text-foreground"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                <FileText size={13} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {lecture.transcript_path
                ? transcriptVisible
                  ? "Hide transcript"
                  : "Show transcript"
                : "Download transcript"}
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => setCaptionsEnabled((v) => !v)}
                disabled={!lecture.transcript_path}
                aria-label={captionsEnabled ? "Hide captions" : "Show captions"}
                className={cn(
                  captionsEnabled
                    ? "text-foreground"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                {captionsEnabled ? (
                  <Subtitles size={13} />
                ) : (
                  <SubtitlesSlash size={13} />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {captionsEnabled ? "Hide captions" : "Show captions"}
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={handleToggleFullscreen}
                aria-label={isFullscreen ? "Exit fullscreen" : "Fullscreen"}
                className="text-muted-foreground hover:text-foreground"
              >
                {isFullscreen ? <ArrowsIn size={13} /> : <ArrowsOut size={13} />}
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {isFullscreen ? "Exit fullscreen (F)" : "Fullscreen (F)"}
            </TooltipContent>
          </Tooltip>
        </div>
      </div>

      {/* Transcript panel */}
      {transcriptVisible && cues.length > 0 && (
        <div className="h-44 shrink-0 border-t border-border bg-background flex flex-col">
          <div className="px-3 h-8 flex items-center border-b border-border shrink-0">
            <span className="text-[11px] font-semibold text-foreground">
              Transcript
            </span>
          </div>
          <div
            ref={transcriptRef}
            className="flex-1 overflow-y-auto px-3 py-2 space-y-0.5"
          >
            {cues.map((cue, i) => (
              <button
                key={i}
                data-cue={i}
                onClick={() => {
                  if (videoRef.current) videoRef.current.currentTime = cue.start;
                }}
                className={cn(
                  "w-full text-left text-[11px] px-2 py-1 rounded flex gap-2 items-start transition-colors",
                  i === activeCueIdx
                    ? "bg-primary/10 text-primary"
                    : "text-muted-foreground hover:text-foreground hover:bg-surface",
                )}
              >
                <span className="font-mono text-[10px] shrink-0 pt-px w-10 text-right opacity-60">
                  {fmtTime(Math.floor(cue.start))}
                </span>
                <span className="flex-1">{cue.text}</span>
                <CaretRight size={10} className="shrink-0 mt-1 opacity-40" />
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
