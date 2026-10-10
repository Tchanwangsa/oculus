import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";

/** The control bar's fade: shown while paused or held, hidden after idle while
 *  playing. Three independent holds pin it — pointer on it, a panel open, a
 *  volume drag that has left it. */
export function useControlsVisibility(
  isPlaying: boolean,
  videoRef: RefObject<HTMLVideoElement | null>,
) {
  /** Controls are over the frame, so they fade out of the way while playing. */
  const [controlsVisible, setControlsVisible] = useState(true);
  const hideControlsRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pointerOnControlsRef = useRef(false);
  const panelOpenRef = useRef(false);
  const volumeDraggingRef = useRef(false);
  const controlsHeld = () =>
    pointerOnControlsRef.current ||
    panelOpenRef.current ||
    volumeDraggingRef.current;

  // Playing, the bar fades after idle seconds; paused, it stays. The timer reads
  // the element, not `isPlaying`, so it never needs re-arming.
  const revealControls = useCallback(() => {
    setControlsVisible(true);
    if (hideControlsRef.current) clearTimeout(hideControlsRef.current);
    hideControlsRef.current = setTimeout(() => {
      if (controlsHeld()) return;
      if (videoRef.current && !videoRef.current.paused) setControlsVisible(false);
    }, 2200);
  }, []);

  useEffect(() => {
    if (isPlaying) revealControls();
    else {
      if (hideControlsRef.current) clearTimeout(hideControlsRef.current);
      setControlsVisible(true);
    }
  }, [isPlaying, revealControls]);

  useEffect(
    () => () => {
      if (hideControlsRef.current) clearTimeout(hideControlsRef.current);
    },
    [],
  );

  /** A popover on the bar opened or closed: an open one pins the bar. */
  const onPanelOpenChange = useCallback(
    (open: boolean) => {
      panelOpenRef.current = open;
      revealControls();
    },
    [revealControls],
  );

  /** The pointer and the volume drag's holds on the bar, for the view. */
  const holds = useMemo(
    () => ({
      leaveArea: () => {
        pointerOnControlsRef.current = false;
        if (controlsHeld()) return;
        if (videoRef.current && !videoRef.current.paused) {
          setControlsVisible(false);
        }
      },
      enterBar: () => {
        pointerOnControlsRef.current = true;
        revealControls();
      },
      leaveBar: () => {
        pointerOnControlsRef.current = false;
        revealControls();
      },
      volumeDragging: (dragging: boolean) => {
        volumeDraggingRef.current = dragging;
        revealControls();
      },
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [revealControls],
  );

  return { controlsVisible, revealControls, onPanelOpenChange, holds };
}
