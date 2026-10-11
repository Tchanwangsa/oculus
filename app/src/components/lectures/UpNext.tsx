import { useCallback, useEffect, useRef, useState } from "react";
import { CircleNotch, DownloadSimple, Play, X } from "@phosphor-icons/react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { getNextLecture, type Lecture } from "@/lib/db";
import { fmtDurationSecs, fmtLectureDate } from "@/lib/lectures";
import {
  LECTURE_DOWNLOADED_EVENT,
  dlKey,
  downloadLecture,
  isDownloading,
  useLectureDownloads,
} from "@/stores/lectures/lectureDownloadStore";

/**
 * Up Next: the card in the lecture player's frame once the playhead passes
 * the lecture's end (`upNextFrom` in `lib/lectures/end.ts`). See docs/viewers.md.
 */

/** Seconds the Play pill fills for after the file ends, before it plays. */
const COUNTDOWN_S = 8;

export type NextLecture = Lecture & { subject_code: string };

/** The lecture after `lecture` in its subject, re-read when it downloads.
 *  Tagged with the lecture it follows, so a route change never shows the old
 *  answer — the new lecture itself — for a render. */
export function useNextLecture(lecture: Lecture): NextLecture | null {
  const [found, setFound] = useState<{ after: string; next: NextLecture | null } | null>(null);
  const { id, subject_id, date } = lecture;
  const idRef = useRef(id);
  idRef.current = id;

  const load = useCallback(() => {
    const settle = (next: NextLecture | null) =>
      idRef.current === id && setFound({ after: id, next });
    getNextLecture({ subject_id, date }).then(settle, () => settle(null));
  }, [id, subject_id, date]);

  useEffect(load, [load]);

  const next = found?.after === id ? found.next : null;
  useWindowEvent(LECTURE_DOWNLOADED_EVENT, (e) => {
    if ((e as CustomEvent<string>).detail === next?.id) load();
  });

  return next;
}

/**
 * The element has played to its end and nothing has happened since — what
 * the countdown waits for. A play, seek, pause or new source clears it. Keyed
 * by lecture too: one element serves every lecture.
 */
export function useEnded(element: HTMLVideoElement | null, lectureId: string): boolean {
  const [ended, setEnded] = useState(false);
  useEffect(() => {
    setEnded(false);
    if (!element) return;
    const on = () => setEnded(true);
    const off = () => setEnded(false);
    const clears = ["play", "seeking", "pause", "emptied"];
    element.addEventListener("ended", on);
    for (const t of clears) element.addEventListener(t, off);
    return () => {
      element.removeEventListener("ended", on);
      for (const t of clears) element.removeEventListener(t, off);
    };
  }, [element, lectureId]);
  return ended;
}

/** Bottom-right in the video frame, above the control bar while it shows.
 *  Colours are fixed, as the bar's are: it sits on the picture. */
export function UpNextCard({
  next,
  ended,
  liftedAbove,
  onPlay,
  onDismiss,
}: {
  next: NextLecture;
  /** The file has ended: the Play pill counts down, then plays. */
  ended: boolean;
  /** The control bar shows, so the card sits above it. */
  liftedAbove: boolean;
  onPlay: () => void;
  onDismiss: () => void;
}) {
  const downloaded = !!(next.video_path || next.video2_path);
  const downloading = useLectureDownloads((s) => isDownloading(s, next.id));
  const percent = useLectureDownloads((s) => s.progress[dlKey(next.id, 1)]?.percent ?? 0);
  const [failed, setFailed] = useState(false);

  const [thumb, setThumb] = useState<string | null>(null);
  useEffect(() => {
    setThumb(null);
    if (!downloaded) return;
    let stale = false;
    invoke<string | null>("lecture_thumbnail", { lectureId: next.id })
      .then((path) => !stale && setThumb(path ? convertFileSrc(path) : null))
      .catch((e) => console.error(`[oculus] thumbnail for ${next.id}:`, e));
    return () => {
      stale = true;
    };
  }, [next.id, downloaded]);

  const counting = ended && downloaded;
  const [left, setLeft] = useState(COUNTDOWN_S);
  const fillRef = useRef<HTMLSpanElement>(null);
  const playRef = useRef(onPlay);
  playRef.current = onPlay;

  useEffect(() => {
    setLeft(COUNTDOWN_S);
    if (!counting) return;
    const started = Date.now();
    const tick = setInterval(() => {
      setLeft(Math.max(0, COUNTDOWN_S - Math.floor((Date.now() - started) / 1000)));
    }, 250);
    const done = setTimeout(() => playRef.current(), COUNTDOWN_S * 1000);
    // Hidden under `prefers-reduced-motion`, where the seconds show instead.
    const fill = fillRef.current?.animate(
      [{ transform: "scaleX(0)" }, { transform: "scaleX(1)" }],
      { duration: COUNTDOWN_S * 1000, easing: "linear", fill: "forwards" },
    );
    return () => {
      clearInterval(tick);
      clearTimeout(done);
      fill?.cancel();
    };
  }, [counting]);

  const download = () => {
    setFailed(false);
    downloadLecture(next, 1).catch(() => setFailed(true));
  };

  return (
    <div
      className={cn(
        "absolute bottom-3 right-3 z-35 flex w-80 max-w-[calc(100%-1.5rem)] cursor-default gap-3 p-2.5",
        "rounded-xl border border-white/15 bg-black/80 text-white shadow-2xl shadow-black/60 backdrop-blur-md",
        "transition-transform duration-200",
        liftedAbove && "-translate-y-17",
      )}
    >
      <div className="relative flex aspect-video w-28 shrink-0 items-center justify-center self-start overflow-hidden rounded-md bg-white/10">
        {thumb ? (
          <img src={thumb} alt="" draggable={false} className="h-full w-full object-cover" />
        ) : (
          <SubjectIcon code={next.subject_code} size={22} />
        )}
      </div>

      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex items-center gap-2">
          <span className="min-w-0 flex-1 text-[11px] font-medium text-white/60">Up next</span>
          <Button
            size="icon-xs"
            variant="ghost"
            aria-label="Dismiss"
            onClick={onDismiss}
            className="-mr-1 -mt-1 text-white/70 hover:bg-white/15 hover:text-white dark:hover:bg-white/15"
          >
            <X />
          </Button>
        </div>
        <p className="line-clamp-2 text-[13px] font-medium leading-snug">{next.title}</p>
        <p className="mt-0.5 text-[11px] tabular-nums text-white/60">
          {fmtLectureDate(next.date)} · {fmtDurationSecs(next.duration_seconds)}
        </p>

        <div className="mt-2 flex items-center gap-2">
          {downloaded ? (
            <Button size="xs" onClick={onPlay} className="relative overflow-hidden">
              <span
                ref={fillRef}
                aria-hidden
                className={cn(
                  "absolute inset-0 origin-left bg-primary-active motion-reduce:hidden",
                  !counting && "hidden",
                )}
                // Not a `scale-x-*` class: that sets `scale`, which the
                // animation's `transform` would multiply rather than replace.
                style={{ transform: "scaleX(0)" }}
              />
              <Play weight="fill" className="relative" />
              <span className="relative">Play</span>
              {counting && (
                <span className="relative hidden tabular-nums motion-reduce:inline">in {left}</span>
              )}
            </Button>
          ) : (
            <Button
              size="xs"
              variant="outline"
              disabled={downloading}
              onClick={download}
              className="border-white/20 bg-white/10 text-white hover:bg-white/20 hover:text-white dark:border-white/20 dark:bg-white/10 dark:hover:bg-white/20"
            >
              {downloading ? <CircleNotch className="animate-spin" /> : <DownloadSimple />}
              {downloading ? (
                <span className="tabular-nums">Downloading {percent}%</span>
              ) : (
                "Download"
              )}
            </Button>
          )}
          {failed && !downloading && (
            <span className="min-w-0 truncate text-[11px] text-white/60">Download failed</span>
          )}
        </div>
      </div>
    </div>
  );
}
