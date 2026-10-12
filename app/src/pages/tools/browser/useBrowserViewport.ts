import { useEffect, useRef } from "react";
import { browser } from "@/lib/browser";
import { useNativeHole } from "@/hooks/shell/useNativeHole";
import { measure } from "./viewport";

/**
 * Where the native page sits: placed at the slot while the tab is in
 * front, with a hole cut over it once Rust has placed it; hidden otherwise,
 * the hole closed first. Portals draw over the live page, so nothing else
 * hides it.
 */
export function useBrowserViewport(id: number, active: boolean) {
  const slotRef = useRef<HTMLDivElement>(null);
  const hole = useNativeHole(slotRef);
  const hidden = !active;

  useEffect(() => {
    const slot = slotRef.current;
    if (!slot || !Number.isInteger(id)) return;
    if (hidden) {
      hole.close();
      browser.hideTab(id).catch(() => {});
      return;
    }
    let live = true;
    browser.place(id, measure(slot)).then(
      () => live && hole.open(),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [id, hidden, hole]);

  // On unmount or id change, hide the outgoing page.
  useEffect(
    () => () => {
      hole.close();
      browser.hideTab(id).catch(() => {});
    },
    [id, hole],
  );

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

  return slotRef;
}
