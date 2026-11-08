import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import type { ErrorPhase } from "../../../../widget-kit/protocol";
import { readWidgetMessage, sendReply, sendVisibility, visibilityToPost, type WidgetRequest } from "./messages";
import type { AskReply } from "./ask/useWidgetAsk";

export interface FrameReport {
  /** The content's height, once the frame has reported it. */
  height: number | null;
  /** The newest error the frame posted. */
  error: { phase: ErrorPhase; message: string } | null;
  /** Content wider than the frame. */
  wide: { width: number; frameWidth: number } | null;
}

/** The report; whether `doc`'s runtime has spoken (its first height), until
 *  which a message to it would be lost; and `forget`, which drops the errors
 *  and overflow reported so far, keeping the height. A streaming frame
 *  forgets just before posting its committed program, so that program's own
 *  reports, queued after, are the ones that count. */
export type FrameState = FrameReport & { spoken: boolean; forget: () => void };

const EMPTY: FrameReport = { height: null, error: null, wide: null };

/**
 * What one widget frame has said since it loaded `doc`: its height, its
 * newest error, horizontal overflow. State goes to `onState`. Only messages
 * whose source is this frame's window count (`readWidgetMessage`). No
 * timeouts: a frame that never reports stays `height: null`, and a kit that
 * never loads is reported by the frame's own guard script on `load`.
 *
 * `live` (in view, not covered by the expanded copy) is posted as
 * `resume`/`pause` once the document's first height report shows its runtime
 * is listening, and again whenever it changes (`visibilityToPost`).
 *
 * An `ask` or `send` goes to `onRequest` with a `reply` bound to this
 * frame's window as it was when asked, so an answer never reaches a
 * rebuilt document.
 */
export function useWidgetFrame(
  frame: RefObject<HTMLIFrameElement | null>,
  doc: string | null,
  onState: (state: Record<string, unknown>) => void,
  initialHeight: number | null = null,
  live = true,
  onRequest?: (req: WidgetRequest, reply: AskReply) => void,
): FrameState {
  const [report, setReport] = useState<FrameReport>({ ...EMPTY, height: initialHeight });
  // The document that has sent a height, and the flag last posted to which
  // document: both per document, so a rebuilt frame gets its own handshake.
  const [spokenDoc, setSpokenDoc] = useState<string | null>(null);
  const sent = useRef<{ doc: string; live: boolean } | null>(null);
  const docRef = useRef(doc);
  docRef.current = doc;
  const stateRef = useRef(onState);
  stateRef.current = onState;
  const requestRef = useRef(onRequest);
  requestRef.current = onRequest;

  const forget = useCallback(() => setReport((r) => ({ ...EMPTY, height: r.height })), []);
  // A new document starts over, keeping the last height so the reply doesn't jump.
  useEffect(forget, [doc, forget]);

  useEffect(() => {
    if (doc == null) return;
    const last = sent.current?.doc === doc ? sent.current.live : null;
    const next = visibilityToPost(last, live, spokenDoc === doc);
    if (next == null) return;
    sent.current = { doc, live: next };
    sendVisibility(frame.current?.contentWindow, next);
  }, [frame, doc, live, spokenDoc]);

  useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      const win = frame.current?.contentWindow;
      const m = readWidgetMessage(e, win);
      if (!m) return;
      if (m.kind === "ask" || m.kind === "send") {
        const handle = requestRef.current;
        const reply: AskReply = (result) => sendReply(win, m.id, result);
        if (handle) handle(m, reply);
        else reply({ error: "This widget can't make requests here." });
      } else if (m.kind === "state") stateRef.current(m.state);
      else if (m.kind === "height") {
        setSpokenDoc(docRef.current);
        setReport((r) => (r.height === m.height ? r : { ...r, height: m.height }));
      } else if (m.kind === "error") setReport((r) => ({ ...r, error: { phase: m.phase, message: m.message } }));
      else setReport((r) => ({ ...r, wide: { width: m.width, frameWidth: m.frameWidth } }));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [frame]);

  return { ...report, spoken: doc != null && spokenDoc === doc, forget };
}
