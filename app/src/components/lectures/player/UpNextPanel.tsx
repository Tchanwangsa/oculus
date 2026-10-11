import { useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import type { Lecture } from "@/lib/db";
import { LECTURES_CHANGED_EVENT, lecturePagePath } from "@/lib/lectures";
import { isWatched, upNextFrom } from "@/lib/lectures/end";
import { completeLecture, playOnAdopt } from "@/lib/lectures/playback";
import { UpNextCard, useEnded, useNextLecture } from "@/components/lectures/UpNext";

interface UpNextPanelProps {
  lecture: Lecture;
  /** The element this player has, once it has one. */
  element: HTMLVideoElement | null;
  currentTime: number;
  duration: number;
  /** Another pane has the playback: nothing to move on from here. */
  elsewhere: boolean;
  /** The control bar is showing, so the card lifts clear of it. */
  controlsVisible: boolean;
}

/** The Up Next card once the playhead passes the lecture's end. */
export function UpNextPanel({
  lecture,
  element,
  currentTime,
  duration,
  elsewhere,
  controlsVisible,
}: UpNextPanelProps) {
  const navigate = useNavigate();
  const next = useNextLecture(lecture);
  const ended = useEnded(element, lecture.id);
  /** × hides the card until the lecture is opened again. */
  const [dismissed, setDismissed] = useState<string | null>(null);
  const upNextAt = upNextFrom(lecture, duration);
  const showUpNext =
    !!next &&
    !!element &&
    !elsewhere &&
    dismissed !== lecture.id &&
    upNextAt != null &&
    currentTime >= upNextAt;

  /** Play: this lecture is Done, and this tab moves on to the next, playing. */
  const leavingRef = useRef(false);
  const playNext = async () => {
    if (!next || leavingRef.current) return;
    leavingRef.current = true;
    try {
      await completeLecture(lecture.id);
      window.dispatchEvent(new CustomEvent(LECTURES_CHANGED_EVENT));
      // One already watched starts over rather than at its end.
      const over = !!next.completed || isWatched(next, next.progress_seconds, 0);
      playOnAdopt(next.id, over ? 0 : undefined);
      navigate(lecturePagePath(next), { replace: true });
    } finally {
      leavingRef.current = false;
    }
  };

  if (!showUpNext || !next) return null;
  return (
    <UpNextCard
      key={next.id}
      next={next}
      ended={ended}
      liftedAbove={controlsVisible}
      onPlay={playNext}
      onDismiss={() => setDismissed(lecture.id)}
    />
  );
}
