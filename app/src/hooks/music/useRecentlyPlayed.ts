import { useCallback, useEffect, useRef, useState } from "react";
import { music, type MusicPlayed } from "@/lib/music";

/**
 * The library's Recently played: asked for the first time `shown` is true
 * while connected, then kept for the mount (Rust keeps Spotify's list across
 * mounts until a refresh). `load(true)` asks Spotify again; a failed load
 * keeps the rows already shown beside its error.
 */
export function useRecentlyPlayed(shown: boolean, connected: boolean) {
  const [played, setPlayed] = useState<MusicPlayed[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const asked = useRef(false);
  const seq = useRef(0);

  const load = useCallback((refresh: boolean) => {
    const mine = ++seq.current;
    setLoading(true);
    setError(null);
    music.recentlyPlayed(refresh).then(
      (list) => {
        if (mine !== seq.current) return;
        setPlayed(list);
        setLoading(false);
      },
      (e: unknown) => {
        if (mine !== seq.current) return;
        setError(String(e));
        setLoading(false);
      },
    );
  }, []);

  useEffect(() => {
    if (!shown || !connected || asked.current) return;
    asked.current = true;
    load(false);
  }, [shown, connected, load]);

  return { played, loading, error, load };
}
