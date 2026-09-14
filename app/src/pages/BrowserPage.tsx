import { useCallback, useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { ArrowClockwise, ArrowSquareOut } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { browser, normalizeAddress, type Viewport } from "@/lib/browser";
import { useBrowserStore } from "@/stores/browserStore";

/**
 * The `/browse/:id` route: an address bar across the top of the content
 * card and, under it, an empty slot that the tab's native page WebView sits
 * in. The page is not in the DOM — Rust parks a WKWebView over the slot
 * (`app/src-tauri/src/browser.rs`) — so this component's job is to say where
 * the slot is, and to step the page aside when the app has to draw over it.
 *
 * Everything shown here comes from Rust's snapshot; the only local state is
 * the address bar's draft while it is being typed in.
 */

function appZoom(): number {
  const z = parseFloat(
    document.documentElement.style.getPropertyValue("--app-zoom"),
  );
  return Number.isFinite(z) && z > 0 ? z : 1;
}

/** The slot's place in the window as insets, in logical points: CSS pixels
 *  times the page zoom. The card's inner corner radius rides along so the
 *  page can round its bottom corners to match. */
function measure(slot: HTMLElement): Viewport {
  const z = appZoom();
  const r = slot.getBoundingClientRect();
  let radius = 0;
  const card = slot.closest("main");
  if (card) {
    const cs = getComputedStyle(card);
    radius = Math.max(
      0,
      parseFloat(cs.borderBottomLeftRadius) - parseFloat(cs.borderLeftWidth),
    );
  }
  return {
    left: r.left * z,
    top: r.top * z,
    right: (window.innerWidth - r.right) * z,
    bottom: (window.innerHeight - r.bottom) * z,
    radius: radius * z,
  };
}

function overlaps(a: DOMRect, b: DOMRect): boolean {
  return (
    a.width > 0 &&
    a.height > 0 &&
    a.left < b.right &&
    a.right > b.left &&
    a.top < b.bottom &&
    a.bottom > b.top
  );
}

/** Whether anything portalled out of the app tree — a popover, tooltip,
 *  menu, dialog — currently lands over the slot. A portal's own wrapper is
 *  an unstyled div, so its children are what get measured. */
function coveredBy(slot: HTMLElement): boolean {
  const page = slot.getBoundingClientRect();
  for (const portal of document.body.children) {
    if (portal.id === "root" || !(portal instanceof HTMLElement)) continue;
    for (const el of [portal, ...portal.children]) {
      if (overlaps(el.getBoundingClientRect(), page)) return true;
    }
  }
  return false;
}

export default function BrowserPage() {
  const params = useParams();
  const id = Number(params.id);
  const tab = useBrowserStore((s) => s.tabs.find((t) => t.id === id));
  const slotRef = useRef<HTMLDivElement>(null);
  const addressRef = useRef<HTMLInputElement>(null);
  const [address, setAddress] = useState(tab?.url ?? "");
  const editingRef = useRef(false);
  const coveredRef = useRef(false);

  // Put this tab's page in the slot. Only this page: Rust places what it is
  // told and nothing else, so whatever else is on screen stays put.
  useEffect(() => {
    const slot = slotRef.current;
    if (!slot || !Number.isInteger(id) || coveredRef.current) return;
    browser.place(id, measure(slot)).catch(() => {});
  }, [id]);

  // Leaving the slot takes the page with it — on unmount for an app tab, and
  // on an id change, which is the same slot handed to another tab. Keyed on
  // the id it put there, so it is the outgoing page that goes down.
  useEffect(() => () => void browser.hideTab(id).catch(() => {}), [id]);

  // The slot moves when the sidebar toggles or the zoom changes (page zoom
  // reflows the viewport, so this fires for it too); the window's own
  // resizes Rust follows without us.
  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) return;
    const observer = new ResizeObserver(() => {
      browser.setViewport(id, measure(slot)).catch(() => {});
    });
    observer.observe(slot);
    return () => observer.disconnect();
  }, [id]);

  // A native view cannot interleave with the DOM: anything the app draws
  // over the page — a sidebar popover, a tooltip reaching in, a dialog —
  // would render beneath it. So watch for portals landing over the slot
  // and hide the page for as long as one is there. Measured a frame after
  // the mutation, once the popper has positioned itself.
  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) return;
    let frame = 0;
    const check = () => {
      frame = 0;
      const covered = coveredBy(slot);
      if (covered === coveredRef.current) return;
      coveredRef.current = covered;
      (covered ? browser.hideTab(id) : browser.place(id, measure(slot))).catch(
        () => {},
      );
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

  // The address bar shows the tab's URL unless it is being typed in.
  useEffect(() => {
    if (!editingRef.current) setAddress(tab?.url ?? "");
  }, [tab?.url, id]);

  const go = useCallback(() => {
    if (!tab) return;
    const target = normalizeAddress(address);
    if (!target) return;
    setAddress(target);
    browser.navigate(tab.id, target).catch(() => {});
  }, [tab, address]);

  // Shortcuts, while the app (not the page) has focus. Back and forward are
  // the tab strip's arrows, which drive the page while a browser tab is
  // in front; ⌘[ and ⌘] reach them from here. ⌘W is not among them: it is a
  // menu item (`app/src-tauri/src/menu.rs`), so it also works while the page
  // has focus and no key event reaches this webview at all.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || !tab) return;
      switch (e.key) {
        case "l":
          e.preventDefault();
          addressRef.current?.focus();
          break;
        case "r":
          e.preventDefault();
          browser.reload(tab.id).catch(() => {});
          break;
        case "[":
          e.preventDefault();
          browser.history(tab.id, -1).catch(() => {});
          break;
        case "]":
          e.preventDefault();
          browser.history(tab.id, 1).catch(() => {});
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [tab]);

  const barButton =
    "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

  return (
    <div className="flex h-full flex-col">
      {/* The hairline under the toolbar is where the page begins. */}
      <div className="flex h-10 shrink-0 items-center gap-1 border-b border-border px-2">
        <button
          onClick={() => tab && browser.reload(tab.id)}
          disabled={!tab}
          aria-label="Reload"
          className={cn(barButton, "mr-1")}
        >
          <ArrowClockwise size={15} className={cn(tab?.loading && "animate-spin")} />
        </button>
        <input
          ref={addressRef}
          value={address}
          disabled={!tab}
          onChange={(e) => setAddress(e.target.value)}
          onFocus={(e) => {
            editingRef.current = true;
            e.currentTarget.select();
          }}
          onBlur={() => {
            editingRef.current = false;
            setAddress(tab?.url ?? "");
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.currentTarget.blur();
              go();
            }
            if (e.key === "Escape") {
              setAddress(tab?.url ?? "");
              e.currentTarget.blur();
            }
          }}
          spellCheck={false}
          autoComplete="off"
          className="h-7 min-w-0 flex-1 rounded-full bg-secondary px-3.5 text-[12.5px] text-foreground outline-none placeholder:text-muted-foreground focus:bg-card focus:ring-2 focus:ring-brand/40 disabled:opacity-50"
          placeholder="Search or enter address"
        />
        <button
          onClick={() => tab && browser.external(tab.url)}
          disabled={!tab}
          aria-label="Open in default browser"
          className={cn(barButton, "ml-1")}
        >
          <ArrowSquareOut size={15} />
        </button>
      </div>
      {/* The page's slot. Nothing renders here; the native view covers it. */}
      <div ref={slotRef} className="min-h-0 flex-1" />
    </div>
  );
}
