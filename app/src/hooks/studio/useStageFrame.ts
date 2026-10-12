import { useEffect, useRef, useState } from "react";

import { componentFrame } from "@/lib/studio/components";
import type { PropValues } from "@/lib/studio/props";

/** How long an edit (props, theme, size) waits for the next before it is
 *  drawn; a move of the playhead does not wait. */
const EDIT_WAIT_MS = 150;

export interface FrameRequest {
  name: string;
  subject: string | null;
  theme: string;
  /** An unsaved theme's text to draw in instead of the saved `theme`. */
  themeText?: string | null;
  props: PropValues;
  t: number;
  width: number;
  duration: number;
  /** Bumped when the component itself changed, so the same request draws again. */
  revision: number;
}

export interface StageFrame {
  /** The newest frame that drew; stays up while the next one is made or fails. */
  url: string | null;
  /** The backend's sentence for why the latest request could not be drawn. */
  error: string | null;
  /** A request is in flight. */
  pending: boolean;
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/**
 * Draws `request` through `studio_component_frame`, one request at a time.
 * While one is in flight only the newest later request is kept, so playing
 * or scrubbing skips the moments it is too slow for and never queues.
 * Edits other than `t` are debounced.
 */
export function useStageFrame(request: FrameRequest | null): StageFrame {
  const [frame, setFrame] = useState<StageFrame>({ url: null, error: null, pending: false });
  const waiting = useRef<FrameRequest | null>(null);
  const busy = useRef(false);
  const alive = useRef(true);
  const drawn = useRef<string | null>(null);
  const pump = useRef<() => void>(() => {});

  pump.current = () => {
    const next = waiting.current;
    if (busy.current || !next) return;
    waiting.current = null;
    busy.current = true;
    setFrame((f) => ({ ...f, pending: true }));
    componentFrame(next.name, {
      subject: next.subject,
      theme: next.theme,
      themeText: next.themeText,
      props: next.props,
      t: next.t,
      width: next.width,
      duration: next.duration,
    })
      .then((blob) => {
        if (!alive.current) return;
        const url = URL.createObjectURL(blob);
        const old = drawn.current;
        drawn.current = url;
        setFrame((f) => ({ ...f, url, error: null }));
        // After React has swapped the image over.
        if (old) setTimeout(() => URL.revokeObjectURL(old), 1000);
      })
      .catch((e) => alive.current && setFrame((f) => ({ ...f, error: message(e) })))
      .finally(() => {
        busy.current = false;
        if (!alive.current) return;
        if (waiting.current) pump.current();
        else setFrame((f) => ({ ...f, pending: false }));
      });
  };

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      if (drawn.current) URL.revokeObjectURL(drawn.current);
    };
  }, []);

  const editKey = request ? JSON.stringify({ ...request, t: 0 }) : null;
  const lastEdit = useRef<string | null>(null);
  const latest = useRef(request);
  latest.current = request;
  const key = request ? `${editKey}@${request.t}` : null;

  useEffect(() => {
    const req = latest.current;
    if (!req) {
      waiting.current = null;
      return;
    }
    const send = () => {
      lastEdit.current = editKey;
      waiting.current = req;
      pump.current();
    };
    if (lastEdit.current == null || lastEdit.current === editKey) {
      send();
      return;
    }
    const id = setTimeout(send, EDIT_WAIT_MS);
    return () => clearTimeout(id);
    // `key` is the request; `latest` only carries it into the effect.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  return frame;
}
