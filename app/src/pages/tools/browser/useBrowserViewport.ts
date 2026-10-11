import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { browser } from "@/lib/browser";
import { coveredBy, measure } from "./viewport";

interface BrowserViewportInput {
  id: number;
  active: boolean;
  /** The suggestion list is open over the page. */
  listOpen: boolean;
  /** The address field is being edited with a still held. */
  holding: boolean;
  /** Blob URL once Rust answers, `null` if it cannot, `undefined` while
   *  pending — which also holds the list closed. */
  still: string | null | undefined;
  setStill: Dispatch<SetStateAction<string | null | undefined>>;
}

/**
 * Where the native page sits: parks it over the slot, hides it when anything
 * covers it, and captures the PNG still that stands in while it is hidden.
 */
export function useBrowserViewport({
  id,
  active,
  listOpen,
  holding,
  still,
  setStill,
}: BrowserViewportInput) {
  const slotRef = useRef<HTMLDivElement>(null);
  const [covered, setCovered] = useState(false);
  const [standing, setStanding] = useState(false);
  const stillSeq = useRef(0);

  // One rule, four reasons — see the page header.
  const hidden = !active || covered || listOpen || holding;

  useEffect(() => {
    const slot = slotRef.current;
    if (!slot || !Number.isInteger(id)) return;
    if (hidden) {
      browser.hideTab(id).catch(() => {});
      return;
    }
    // Drop the still only once the live page is back, else one blank frame.
    browser
      .place(id, measure(slot))
      .catch(() => {})
      .finally(() => {
        setStanding(false);
        setStill(undefined);
      });
  }, [id, hidden]);

  // On unmount or id change, hide the outgoing page.
  useEffect(() => () => void browser.hideTab(id).catch(() => {}), [id]);

  // Sidebar toggles and page zoom move the slot; Rust follows window resizes
  // itself.
  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) return;
    const observer = new ResizeObserver(() => {
      browser.setViewport(id, measure(slot)).catch(() => {});
    });
    observer.observe(slot);
    return () => observer.disconnect();
  }, [id]);

  // Measured a frame after the mutation, once the popper has positioned.
  // Only portals count, so the suggestion list cannot feed back into this.
  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) return;
    let frame = 0;
    const check = () => {
      frame = 0;
      setCovered(coveredBy(slot));
    };
    const observer = new MutationObserver(() => {
      if (!frame) frame = requestAnimationFrame(check);
    });
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["style", "data-state"],
    });
    return () => {
      observer.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, [id]);

  // Taken on focus, a keystroke ahead of the list; sequenced so a late
  // snapshot of a page you have left is dropped.
  const captureStill = useCallback(() => {
    if (!Number.isInteger(id)) return;
    const seq = ++stillSeq.current;
    setStill(undefined);
    browser
      .snapshot(id)
      .then((png) => {
        if (stillSeq.current !== seq) return;
        setStill(URL.createObjectURL(new Blob([png], { type: "image/png" })));
      })
      .catch(() => {
        if (stillSeq.current === seq) setStill(null);
      });
  }, [id]);

  // Revoke each blob URL once `still` moves on.
  useEffect(() => {
    if (typeof still !== "string") return;
    return () => URL.revokeObjectURL(still);
  }, [still]);

  // `listOpen` already shows the still in the render that opens the list;
  // `standing` keeps it up after the field blurs, until the live page is back.
  useEffect(() => {
    if ((listOpen || holding) && typeof still === "string") setStanding(true);
  }, [listOpen, holding, still]);

  return { slotRef, standing, captureStill };
}
