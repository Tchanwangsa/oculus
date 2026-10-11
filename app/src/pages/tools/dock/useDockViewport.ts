import { useEffect, useRef, useState } from "react";
import { dock, type DockServiceId } from "@/lib/dock";
import { coveredBy, measure } from "@/pages/tools/browser/viewport";

/** The pane that last placed each service. Two panes can route to one
 *  service; only the one showing it may move or park it, so an outgoing
 *  pane's park can't land after the incoming pane's place. */
const holders = new Map<DockServiceId, number>();

function release(service: DockServiceId, paneId: number) {
  if (holders.get(service) !== paneId) return;
  holders.delete(service);
  dock.park(service).catch(() => {});
}

/**
 * Where a dock page sits: placed over the slot while its tab is in front and
 * nothing covers it, parked otherwise. Parking moves the page off-screen and
 * keeps it running; the browser's hook hides instead, which throttles timers.
 */
export function useDockViewport(service: DockServiceId, paneId: number, active: boolean) {
  const slotRef = useRef<HTMLDivElement>(null);
  const [covered, setCovered] = useState(false);
  const parked = !active || covered;

  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) return;
    if (parked) {
      release(service, paneId);
      return;
    }
    holders.set(service, paneId);
    dock.place(service, measure(slot)).catch(() => {});
  }, [service, paneId, parked]);

  useEffect(() => () => release(service, paneId), [service, paneId]);

  // Sidebar toggles and page zoom move the slot; Rust follows window resizes
  // itself.
  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) return;
    const observer = new ResizeObserver(() => {
      if (holders.get(service) !== paneId) return;
      dock.setViewport(service, measure(slot)).catch(() => {});
    });
    observer.observe(slot);
    return () => observer.disconnect();
  }, [service, paneId]);

  // A portal over the slot parks the page: nothing draws over a native view.
  // Measured a frame after the mutation, once the popper has positioned.
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
  }, [service]);

  return slotRef;
}
