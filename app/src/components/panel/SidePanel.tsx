import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { useResizablePanel } from "@/hooks/useResizablePanel";
import { useTabStore } from "@/stores/tabStore";
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

/** How long the panel takes to slide in or out. Must match the
 *  `duration-150` on the width transition below — this is the timer that
 *  decides when the contents may finally be thrown away. */
const ANIM_MS = 150;

/** How long the panel takes to sweep out to the full card when its contents
 *  are promoted to a page in the same tab. Longer than the open/close slide
 *  because it crosses the whole card rather than a panel's width, and it is
 *  the only animation that has to read as *this thing became the page*. */
const EXPAND_MS = 260;

/**
 * The side panel: files and lectures open here, docked against the right of
 * the content card rather than sliding over the page.
 *
 * It is drawn once, as furniture inside the card, and shows whichever item the
 * tab in front has open (`sidePanelStore`). Docked means the page beside it is
 * genuinely narrower — which is what keeps the native browser webview honest,
 * since `BrowserPage` measures its own slot and re-places the webview when the
 * slot resizes, where an overlay could only ever hide it.
 *
 * Width and collapsed are the panel's, not any tab's: dragging it wider in one
 * tab widens it everywhere.
 *
 * Expanding is the same width transition run the other way: the frame sweeps
 * out to the full card and the page underneath is committed behind it, so the
 * peek *becomes* the page instead of vanishing and being replaced. ⌘-click
 * skips the sweep and opens a tab, where the peek is not the thing taking the
 * page over.
 *
 * The frame is always mounted, at zero width when nothing is open, so opening
 * and closing are a width transition on an element that already exists rather
 * than a mount and an unmount — which nothing can animate. What it *draws*
 * lags the store by one animation (`drawn`), so a closing panel still has
 * contents to slide out with.
 */
export function SidePanel() {
  const item = useActivePanelItem();
  const activeId = useTabStore((s) => s.activeId);
  const addTab = useTabStore((s) => s.addTab);
  const close = useSidePanelStore((s) => s.close);
  const panel = useResizablePanel(PANEL);
  const { setCollapsed } = panel;

  // Opening something has to overrule a panel that was left folded, or the
  // click would look like it did nothing at all. Counted rather than watched
  // for a change: re-opening the row that is already showing leaves the item
  // identical, and that click has to unfold the panel too — which is the one
  // way a folded panel could swallow clicks indefinitely.
  const opens = useSidePanelStore((s) => s.opens);
  useEffect(() => {
    if (opens > 0) setCollapsed(false);
  }, [opens, setCollapsed]);

  // What is on screen, which outlives what is in the store by one exit: a
  // panel whose contents vanished the instant it closed would collapse on an
  // empty frame. The tab id rides along rather than being read live, so a
  // panel sliding out of a tab you just left still belongs to the tab that
  // opened it — and dropping the pair at the end is what finally stops a
  // lecture playing.
  const [drawn, setDrawn] = useState<{ item: PanelItem; tabId: number } | null>(
    item && activeId != null ? { item, tabId: activeId } : null,
  );
  useEffect(() => {
    if (item && activeId != null) {
      setDrawn({ item, tabId: activeId });
      return;
    }
    const t = setTimeout(() => setDrawn(null), ANIM_MS);
    return () => clearTimeout(t);
  }, [item, activeId]);

  // Memoised: a new object every render would re-render the panel's body — and
  // the lecture player's would re-run its whole layout reconcile — for nothing.
  const panelTab = useMemo(
    () => ({ id: drawn?.tabId ?? 0, active: true }),
    [drawn?.tabId],
  );

  // The width the frame is sweeping out to while an expansion runs, and null
  // the rest of the time. Measured off the card rather than assumed, because
  // the card is whatever the window and the sidebar have left it.
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
   * Promote the open item to a full page. ⌘-click takes a new tab and leaves
   * this one where it is; a plain click is this tab becoming that page, which
   * is what the sweep draws.
   *
   * The navigation is committed at the *end* of the sweep, not the start: the
   * page mounting underneath a panel that is still sliding would re-render the
   * whole card for every frame of it, and the panel already holds exactly what
   * the page is about to show.
   */
  const expand = useCallback(
    (path: string, newTab: boolean) => {
      const tabId = activeId;
      if (tabId == null) return;
      if (newTab) {
        addTab(path);
        close(tabId);
        return;
      }
      if (expandTimer.current) return;
      const full = frameRef.current?.parentElement?.clientWidth ?? null;
      const commit = () => {
        navigateInTab(tabId, path);
        close(tabId);
        // No exit slide: the page took the panel's place at its full width, so
        // there is nothing left to slide out — dropping `drawn` here is what
        // skips the timer the close would otherwise start.
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
    [activeId, addTab, close],
  );

  useEffect(() => {
    if (!item || activeId == null) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(activeId);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [item, activeId, close]);

  // ⌥⌘S folds the panel away, the third of the app's fold shortcuts after ⌘B
  // for the sidebar and ⌥⌘B for the chat's conversations column — ⌥⌘B being
  // taken is the whole reason this one is S.
  //
  // `e.code`, not `e.key`: on macOS ⌥ rewrites the character the key produces,
  // so ⌥S arrives as `ß` and a `key === "s"` test never fires.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!e.altKey || !(e.metaKey || e.ctrlKey) || e.code !== "KeyS") return;
      e.preventDefault();
      panel.toggle();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [panel.toggle]);

  // Zero the moment the store lets go, while `drawn` lingers — that gap is
  // the exit animation.
  const width =
    expandTo ?? (item && activeId != null ? panel.width : 0);

  return (
    <>
      {/* The grip sits *on* the seam, as a sibling rather than inside the
          panel: `w-1` with a matching negative margin either side costs no
          layout width, and living outside the panel's `overflow-hidden` means
          it neither gets clipped nor vanishes when the panel folds — dragging
          it back out is the second way in, the same trade the chat's
          conversations column makes. */}
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
        {/* Pinned to the panel's right edge at the width it unfolds back to,
            never laid out at the animating one: the contents hold still while
            the frame wipes across them, so opening and closing cost no reflow
            inside — which matters most for the lecture player, whose video
            would otherwise be re-measured every frame of the slide. */}
        {drawn && (
          <div
            className={cn(
              "absolute inset-y-0 right-0 flex flex-col border-l border-border bg-background",
              // Expanding is the one time the contents *do* travel with the
              // frame: the panel is becoming the page, so it grows rather than
              // being wiped across.
              expandTo != null && "left-0",
            )}
            style={{ width: expandTo != null ? undefined : panel.restWidth }}
          >
            {/* The peek belongs to the tab it was opened from, even though it
                is drawn by the shell and sits outside every pane. Without
                this the player inside reads the detached default — tab 0,
                which no tab has — and a playing peek is owned by nobody:
                closing its tab neither stops it nor asks, leaving a lecture
                running with nothing on screen. `active` is unconditionally
                true because only the tab in front ever has a body here. */}
            <TabContext.Provider value={panelTab}>
              {/* Keyed so swapping to a different file or lecture builds a
                  fresh body instead of feeding new props through the old
                  one. */}
              {drawn.item.kind === "file" ? (
                <FilePanel
                  key={itemKey(drawn.item)}
                  file={drawn.item.file}
                  tabId={drawn.tabId}
                  onExpand={expand}
                />
              ) : (
                <LecturePanel
                  key={itemKey(drawn.item)}
                  lecture={drawn.item.lecture}
                  tabId={drawn.tabId}
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
