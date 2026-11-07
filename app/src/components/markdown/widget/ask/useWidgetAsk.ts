import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useWidgetStore, type AskGrant } from "@/stores/chat/widgetStore";
import type { WidgetRequest } from "../messages";
import { ALLOWED, DENIED, gate, spend } from "./gate";
import { REFUSED } from "./limits";

export type AskReply = (result: { value: unknown } | { error: string }) => void;

/** What the widget's chrome shows and does about its requests. */
export interface AskControls {
  /** A request waits for the student's Allow. */
  holding: boolean;
  /** A request was refused at the cap since this mounted. */
  capped: boolean;
  /** An `ask` is with the model. */
  asking: boolean;
  allow: () => void;
  deny: () => void;
  allowMore: () => void;
  stop: () => void;
  /** Takes a frame's `ask` or `send`; `reply` answers that frame. */
  handle: (req: WidgetRequest, reply: AskReply) => void;
}

/** Rust's handle on one `widget_ask`, for `widget_ask_cancel`. Random, not
 *  counted, so a reloaded webview never reuses one still running. */
const newRequestId = () => Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);

/**
 * One widget's `oculus/ask` requests, from its inline frame and its expanded
 * copy alike: `gate` decides, the grant and use count live in its
 * `widgetStore` entry (so a changed source asks again), `ask` runs through
 * `widget_ask` and `send` through the thread's `send`. No timeout: Stop or
 * unmounting cancels, and a reply after unmounting is dropped.
 */
export function useWidgetAsk(storeKey: string, source: string, send: ((text: string) => void) | null): AskControls {
  const [holding, setHolding] = useState(false);
  const [capped, setCapped] = useState(false);
  const [asking, setAsking] = useState(false);
  const busy = useRef(false);
  const held = useRef<{ req: WidgetRequest; reply: AskReply } | null>(null);
  const running = useRef<number | null>(null);
  const alive = useRef(true);
  const sendRef = useRef(send);
  sendRef.current = send;
  const where = useRef({ storeKey, source });
  where.current = { storeKey, source };

  const readGrant = useCallback((): AskGrant | undefined => {
    const { storeKey: key, source: src } = where.current;
    const e = useWidgetStore.getState().entries[key];
    return e?.source === src ? e.ask : undefined;
  }, []);
  const writeGrant = useCallback((grant: AskGrant) => {
    const { storeKey: key, source: src } = where.current;
    useWidgetStore.getState().setAsk(key, src, grant);
  }, []);

  const run = useCallback(
    async (req: WidgetRequest, reply: AskReply) => {
      if (req.kind === "send") {
        busy.current = false;
        const post = sendRef.current;
        if (!post) return reply({ error: REFUSED.noChat });
        writeGrant(spend(readGrant() ?? ALLOWED));
        post(req.text);
        return reply({ value: null });
      }
      writeGrant(spend(readGrant() ?? ALLOWED));
      const requestId = newRequestId();
      running.current = requestId;
      setAsking(true);
      try {
        const value = await invoke<unknown>("widget_ask", { requestId, prompt: req.prompt, schema: req.schema });
        if (alive.current) reply({ value });
      } catch (e) {
        if (alive.current) reply({ error: String(e) });
      } finally {
        running.current = null;
        busy.current = false;
        if (alive.current) setAsking(false);
      }
    },
    [readGrant, writeGrant],
  );

  const handle = useCallback(
    (req: WidgetRequest, reply: AskReply) => {
      const verdict = gate(req, { grant: readGrant(), busy: busy.current, canSend: !!sendRef.current });
      if ("refuse" in verdict) {
        if (verdict.capped) setCapped(true);
        return reply({ error: verdict.refuse });
      }
      busy.current = true;
      if ("hold" in verdict) {
        held.current = { req, reply };
        setHolding(true);
        return;
      }
      void run(req, reply);
    },
    [readGrant, run],
  );

  const allow = useCallback(() => {
    writeGrant(ALLOWED);
    setHolding(false);
    const h = held.current;
    held.current = null;
    if (h) void run(h.req, h.reply);
    else busy.current = false;
  }, [run, writeGrant]);

  const deny = useCallback(() => {
    writeGrant(DENIED);
    setHolding(false);
    const h = held.current;
    held.current = null;
    busy.current = false;
    h?.reply({ error: REFUSED.denied });
  }, [writeGrant]);

  const allowMore = useCallback(() => {
    writeGrant(ALLOWED);
    setCapped(false);
  }, [writeGrant]);

  const stop = useCallback(() => {
    if (running.current != null) void invoke("widget_ask_cancel", { requestId: running.current }).catch(() => {});
  }, []);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      held.current = null;
      busy.current = false;
      if (running.current != null) void invoke("widget_ask_cancel", { requestId: running.current }).catch(() => {});
    };
  }, []);

  return { holding, capped, asking, allow, deny, allowMore, stop, handle };
}
