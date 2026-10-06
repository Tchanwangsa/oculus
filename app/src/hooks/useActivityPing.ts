import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PING_INTERVAL_MS, createThrottle } from "@/lib/usage";
import { sameContext, usageContext, type UsageContext } from "@/lib/usageContext";
import { activePane, panesOf, useTabStore } from "@/stores/tabStore";

type ActivityKind = "input" | "media";

const INPUT_EVENTS = ["pointerdown", "pointermove", "keydown", "wheel"] as const;
const MEDIA_EVENTS = ["play", "pause", "ended"] as const;

/** A context change pings only within this long of real input, so a
 *  navigation nobody made (tabs restoring at launch) isn't credited as use. */
const CONTEXT_INPUT_MS = 5_000;

function ping(kind: ActivityKind, context: UsageContext) {
  invoke("usage_activity", { kind, context }).catch((e) => console.debug("usage_activity", e));
}

/** True while any `<video>`/`<audio>` in the document is playing. */
function mediaPlaying(): boolean {
  return Array.from(document.querySelectorAll<HTMLMediaElement>("video, audio")).some(
    (m) => !m.paused && !m.ended,
  );
}

/** Where the user is: the focused pane of the tab in front. */
function focusedContext(): UsageContext {
  return usageContext(activePane()?.path ?? "");
}

/** The context of the pane whose root (`data-pane-id`, set in `TabPane`)
 *  holds `target`; null outside every pane, e.g. a parked lecture video. */
function paneContext(target: EventTarget | null): UsageContext | null {
  const root = target instanceof Element ? target.closest<HTMLElement>("[data-pane-id]") : null;
  const id = Number(root?.dataset.paneId);
  if (!id) return null;
  for (const tab of useTabStore.getState().tabs) {
    const pane = panesOf(tab).find((p) => p.id === id);
    if (pane) return usageContext(pane.path);
  }
  return null;
}

/**
 * Tells Rust the user is here, and on which page, for the active time behind
 * Home's Activity card: input at most once per interval, and a steady beat
 * while media plays so a lecture watched hands-off still counts. Every ping
 * carries a `usageContext`: input the focused pane's, media the pane hosting
 * the element that started playing. Mounted once, in `AppLayout`.
 */
export function useActivityPing(): void {
  useEffect(() => {
    const inputGate = createThrottle(PING_INTERVAL_MS);
    const mediaGate = createThrottle(PING_INTERVAL_MS);
    let current = focusedContext();
    let media: UsageContext | null = null;
    let beat: ReturnType<typeof setInterval> | null = null;
    let lastInput = -Infinity;

    const onInput = () => {
      lastInput = Date.now();
      if (inputGate()) ping("input", current);
    };

    // Navigating, switching tab or moving focus between main page and side
    // panel pings at once, past the throttle, so the next tick credits the new
    // place rather than the one the last ping named.
    const unsubscribe = useTabStore.subscribe(() => {
      const next = focusedContext();
      if (sameContext(current, next)) return;
      current = next;
      if (Date.now() - lastInput < CONTEXT_INPUT_MS) ping("input", current);
    });

    // Media events don't bubble, hence capture on `document`. The beat re-checks
    // each tick: a removed element pauses without an event reaching us.
    const stop = () => {
      if (beat) clearInterval(beat);
      beat = null;
      media = null;
    };
    const tick = () => (mediaPlaying() && media ? ping("media", media) : stop());
    const onMedia = (e: Event) => {
      if (!mediaPlaying()) return stop();
      if (beat) return;
      // Fixed for the whole playback, so browsing elsewhere while a lecture
      // plays still credits the lecture.
      media = paneContext(e.target) ?? current;
      // Gated so rapid play/pause toggling can't ping on every start.
      if (mediaGate()) ping("media", media);
      beat = setInterval(tick, PING_INTERVAL_MS);
    };

    for (const n of INPUT_EVENTS) window.addEventListener(n, onInput, { passive: true });
    for (const n of MEDIA_EVENTS) document.addEventListener(n, onMedia, true);
    return () => {
      for (const n of INPUT_EVENTS) window.removeEventListener(n, onInput);
      for (const n of MEDIA_EVENTS) document.removeEventListener(n, onMedia, true);
      unsubscribe();
      stop();
    };
  }, []);
}
