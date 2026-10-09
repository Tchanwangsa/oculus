import { useEffect, useState, type RefObject } from "react";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import type { Lecture, SourceNum } from "@/lib/db";
import {
  LECTURE_PROGRESS_EVENT,
  parkLectureVideos,
  playbackClaim,
  releaseLecturePlayer,
  syncLectureSources,
  videoForSource,
  type SourcePlan,
} from "@/lib/lectures/playback";
import { mayAdopt, playbackOwner } from "@/lib/lectures/playbackOwner";
import type { Layout } from "@/stores/lectures/playerPrefsStore";

interface SharedElementArgs {
  lecture: Lecture;
  tabId: number;
  stripTab: number;
  onScreen: boolean;
  /** This player may hold the elements; see `mayAdopt`. */
  adoptable: boolean;
  mainSrc: string | null;
  mainSource: SourceNum;
  otherSource: SourceNum;
  urls: Record<SourceNum, string | null>;
  layout: Layout;
  mainHostRef: RefObject<HTMLDivElement | null>;
  secondHostRef: RefObject<HTMLDivElement | null>;
  /** What the adoption does once a user action claims playback here. */
  startRef: RefObject<{ at?: number } | null>;
  attach: (v: HTMLVideoElement | null) => () => void;
  onRefresh: () => void;
}

/** Moves the long-lived `<video>` elements into this player's frames while it
 *  may hold them, and hands them back when it may not. Returns the inset
 *  picture's own aspect. */
export function useSharedElement({
  lecture,
  tabId,
  stripTab,
  onScreen,
  adoptable,
  mainSrc,
  mainSource,
  otherSource,
  urls,
  layout,
  mainHostRef,
  secondHostRef,
  startRef,
  attach,
  onRefresh,
}: SharedElementArgs) {
  /** The inset picture's own aspect, read in the reconcile effect below. */
  const [pipAspect, setPipAspect] = useState(16 / 9);

  useEffect(() => {
    const mainHost = mainHostRef.current;
    // Behind another tab, or beside the player that has them, the elements
    // stay where they are; this player adopts them once `mayAdopt` says so.
    // Re-checked against the live owner: two players can both pass at render.
    if (
      !mainHost ||
      !mainSrc ||
      !onScreen ||
      !adoptable ||
      !mayAdopt(playbackOwner(), tabId, stripTab, lecture.id)
    ) {
      attach(null);
      return;
    }

    // The main frame is first, and first is the leader: its audio is what plays.
    const plan: SourcePlan[] = [{ source: mainSource, src: mainSrc, host: mainHost }];
    const secondHost = secondHostRef.current;
    const secondSrc = urls[otherSource];
    if (layout !== "single" && secondHost && secondSrc) {
      plan.push({ source: otherSource, src: secondSrc, host: secondHost });
    }

    const start = startRef.current ?? undefined;
    startRef.current = null;
    const v = syncLectureSources(lecture, plan, tabId, stripTab, start);
    // Parking restores the elements only if still ours — another player (e.g.
    // expand-to-tab) may have taken them since.
    const claim = playbackClaim();
    if (!v) return;
    const detach = attach(v);

    // Read here: the inset element exists only once the plan is reconciled.
    const inset = plan.length > 1 ? videoForSource(plan[1].source) : null;
    const readAspect = () => {
      if (inset?.videoWidth && inset.videoHeight) {
        setPipAspect(inset.videoWidth / inset.videoHeight);
      }
    };
    readAspect();
    inset?.addEventListener("loadedmetadata", readAspect);

    return () => {
      detach();
      inset?.removeEventListener("loadedmetadata", readAspect);
      // Unmounting is a tab switch, not a stop: the elements go back to their
      // off-screen host and carry on. Closing the lecture is what stops them.
      parkLectureVideos(claim);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lecture.id, urls[1], urls[2], layout, mainSource, onScreen, adoptable, tabId, stripTab]);

  // Unmounting paused gives playback up, so the other pane's player can take it.
  useEffect(() => () => releaseLecturePlayer(tabId), [tabId]);

  // Progress is saved by the module, even while nothing is mounted.
  useWindowEvent(LECTURE_PROGRESS_EVENT, () => onRefresh());

  return pipAspect;
}
