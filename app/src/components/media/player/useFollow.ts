import { useCallback, useRef, useState } from "react";

/** Whether the dock's list tracks playback. `FollowList` does the scrolling;
 *  this owns the flag and the ways back to live, shared by every list the dock
 *  shows, which are never mounted together. */
export function useFollow() {
  const [following, setFollowing] = useState(true);
  // `following` mirrored for pointer handlers, which must not re-subscribe.
  const followingRef = useRef(true);

  const onScrollAway = useCallback(() => {
    if (!followingRef.current) return;
    followingRef.current = false;
    setFollowing(false);
  }, []);

  const onBackToLive = useCallback(() => {
    if (followingRef.current) return;
    followingRef.current = true;
    setFollowing(true);
  }, []);

  return { following, setFollowing, followingRef, onScrollAway, onBackToLive };
}
