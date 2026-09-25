import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { useResizablePanel } from "@/hooks/useResizablePanel";
import { useActivePaneId, useTabStore } from "@/stores/tabStore";
import { navigateInTab } from "@/lib/tabRouters";
import {
  itemKey,
  useActivePanelItem,
  useSidePanelStore,
  type PanelItem,
} from "@/stores/sidePanelStore";
import { TabContext } from "@/components/tabs/TabContext";
import { ResizeHandle } from "@/components/ui/ResizeHandle";
import FilePanel from "@/components/panel/FilePanel";
import LecturePanel from "@/components/panel/LecturePanel";

const PANEL = {
  defaultWidth: 520,
  minWidth: 360,
  maxWidth: 1100,
  // Docked right: dragging *left* widens it.
  side: "right",
  storageKey: "oculus-side-panel",
} as const;

/** Slide in/out time; must match the `duration-150` width transition below. */
const ANIM_MS = 150;

/** The sweep out to the full card when the item is promoted to a page. */
const EXPAND_MS = 260;

/**
 * The side panel: files and lectures dock against the right of the content
 * card, showing whichever item the focused pane has open (`sidePanelStore`).
 * Docked, not overlaid, so `BrowserPage` can re-place its native webview in
 * the narrower slot. Width and collapsed state are global, not per tab.
 *
 * The frame is always mounted (zero width when empty) so open/close animate
 * as a width transition; `drawn` lags the store by one exit so a closing
 * panel has contents to slide out with.
 */
export function SidePanel() {
  const item = useActivePanelItem();
  // Keyed by the focused pane, not the tab: each split half has its own peek.
  const paneId = useActivePaneId();
  const tabId = useTabStore((s) => s.activeId);
  const addTab = useTabStore((s) => s.addTab);
  const close = useSidePanelStore((s) => s.close);
  const panel = useResizablePanel(PANEL);
  const { setCollapsed } = panel;

  // Any open unfolds a folded panel. A counter, not the item, so re-opening
  // the item already showing still unfolds it.
  const opens = useSidePanelStore((s) => s.opens);
  useEffect(() => {
    if (opens > 0) setCollapsed(false);
  }, [opens, setCollapsed]);

  // What is on screen, outliving the store by one exit. The pane id rides
  // along so a sliding-out panel keeps its owner; dropping it stops a lecture.
  const [drawn, setDrawn] = useState<{ item: PanelItem; paneId: number } | null>(
    item && paneId !== 0 ? { item, paneId } : null,
  );
  useEffect(() => {
    if (item && paneId !== 0) {
      setDrawn({ item, paneId });
      return;
    }
    const t = setTimeout(() => setDrawn(null), ANIM_MS);
    return () => clearTimeout(t);
  }, [item, paneId]);

  // Memoised so the panel body (notably the lecture player) doesn't re-render.
  const panelTab = useMemo(
    () => ({ id: drawn?.paneId ?? 0, tabId, side: "main" as const, active: true }),
    [drawn?.paneId, tabId],
  );

  // The card width an expansion is sweeping out to; null otherwise.
  const frameRef = useRef<HTMLDivElement>(null);
  const [expandTo, setExpandTo] = useState<number | null>(null);
  const expandTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (expandTimer.current) clearTimeout(expandTimer.current);
    },
    [],
  );

  /**
   * Promote the open item to a full page: ⌘-click opens a new tab; a plain
   * click sweeps the panel across the card and navigates this pane at the end
   * of the sweep (mounting the page mid-sweep would re-render every frame).
   */
  const expand = useCallback(
    (path: string, newTab: boolean) => {
      if (paneId === 0) return;
      if (newTab) {
        addTab(path);
        close(paneId);
        return;
      }
      if (expandTimer.current) return;
      const full = frameRef.current?.parentElement?.clientWidth ?? null;
      const commit = () => {
        navigateInTab(paneId, path);
        close(paneId);
        // No exit slide: the page already fills the card.
        setDrawn(null);
        setExpandTo(null);
      };
      if (full == null) {
        commit();
        return;
      }
      setExpandTo(full);
      expandTimer.current = setTimeout(() => {
        expandTimer.current = null;
        commit();
      }, EXPAND_MS);
    },
    [paneId, addTab, close],
  );

  useEffect(() => {
    if (!item || paneId === 0) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(paneId);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [item, paneId, close]);

  // ⌥⌘S folds the panel. `e.code`, not `e.key`: on macOS ⌥S arrives as `ß`.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!e.altKey || !(e.metaKey || e.ctrlKey) || e.code !== "KeyS") return;
      e.preventDefault();
      panel.toggle();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [panel.toggle]);

  // Zero as soon as the store lets go, while `drawn` lingers for the exit.
  const width = expandTo ?? (item && paneId !== 0 ? panel.width : 0);

  return (
    <>
      {/* A sibling on the seam, outside the panel's `overflow-hidden`, so it
          survives folding and can drag the panel back out. */}
      {drawn && expandTo == null && (
        <ResizeHandle
          onMouseDown={panel.onMouseDown}
          dragging={panel.dragging}
          label="Resize side panel"
          className="-mx-0.5"
        />
      )}
      <div
        ref={frameRef}
        role="complementary"
        aria-label="Side panel"
        aria-hidden={!drawn}
        className={cn(
          "relative shrink-0 overflow-hidden",
          !panel.dragging && "transition-[width] duration-150 ease-out",
        )}
        style={{
          width,
          transitionDuration: expandTo != null ? `${EXPAND_MS}ms` : undefined,
        }}
      >
        {/* Pinned right at the rest width so contents don't reflow while the
            frame animates (the lecture video would re-measure every frame). */}
        {drawn && (
          <div
            className={cn(
              "absolute inset-y-0 right-0 flex flex-col border-l border-border bg-background",
              // Expanding: contents grow with the frame instead.
              expandTo != null && "left-0",
            )}
            style={{ width: expandTo != null ? undefined : panel.restWidth }}
          >
            {/* Drawn outside every pane, so give it its opener's pane id —
                otherwise a playing lecture is owned by nobody and survives
                closing its tab. */}
            <TabContext.Provider value={panelTab}>
              {/* Keyed so a different item builds a fresh body. */}
              {drawn.item.kind === "file" ? (
                <FilePanel
                  key={itemKey(drawn.item)}
                  file={drawn.item.file}
                  paneId={drawn.paneId}
                  onExpand={expand}
                />
              ) : (
                <LecturePanel
                  key={itemKey(drawn.item)}
                  lecture={drawn.item.lecture}
                  paneId={drawn.paneId}
                  onExpand={expand}
                />
              )}
            </TabContext.Provider>
          </div>
        )}
      </div>
    </>
  );
}
