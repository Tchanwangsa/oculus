import { useRef, useState } from "react";
import { useParams } from "react-router-dom";
import {
  ArrowClockwise,
  ArrowSquareOut,
  CaretLeft,
  CaretRight,
  Globe,
  MagnifyingGlass,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { browser } from "@/lib/browser";
import { faviconFor } from "@/hooks/shell/useBrowserTabs";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { useBrowserStore } from "@/stores/shell/browserStore";
import { useActivePaneId } from "@/stores/shell/tabStore";
import { useTabActive, useTabId } from "@/components/tabs/TabContext";
import { FindBar } from "@/components/ui/search/FindBar";
import { useBrowserAddressBar } from "./browser/useBrowserAddressBar";
import { useBrowserFind } from "./browser/useBrowserFind";
import { useBrowserViewport } from "./browser/useBrowserViewport";

/**
 * The `/browse/:id` route: a toolbar over an empty slot that Rust parks the
 * tab's native WKWebView over (`app/src-tauri/src/shell/browser/`).
 *
 * A native view cannot sit under the DOM, so anything drawn over the slot must
 * hide the page: a portalled popover, the suggestion list, the tab going to the
 * background. They fold into one `hidden` — two effects disagreeing about one
 * page leave it parked over the app. The suggestion list is drawn over a PNG
 * still of the page (`browser_snapshot`) so the card does not blank.
 */
export default function BrowserPage() {
  const params = useParams();
  const id = Number(params.id);
  const active = useTabActive();
  // `active` is true for both the main pane and the side panel; menu
  // shortcuts go only to the focused one.
  const paneId = useTabId();
  const focusedPaneId = useActivePaneId();
  const focused = active && paneId === focusedPaneId;
  const tab = useBrowserStore((s) => s.tabs.find((t) => t.id === id));
  const favicons = useBrowserStore((s) => s.favicons);
  const rootRef = useRef<HTMLDivElement>(null);
  // Blob URL once Rust answers, `null` if it cannot, `undefined` while
  // pending — which also holds the list closed.
  const [still, setStill] = useState<string | null | undefined>(undefined);

  const {
    addressRef,
    address,
    setAddress,
    setEditing,
    setMatches,
    picked,
    setPicked,
    suggestions,
    listOpen,
    holding,
    goTo,
    onAddressKeyDown,
  } = useBrowserAddressBar(tab, id, still);
  const { slotRef, standing, captureStill } = useBrowserViewport({
    id,
    active,
    listOpen,
    holding,
    still,
    setStill,
  });
  const { findRef, find, setFind, runFind, closeFind, openFind } = useBrowserFind(
    tab,
    id,
    paneId,
    rootRef,
  );

  // ⌘L. Every browser shortcut is a menu item, never a `keydown`:
  // `browser_place` focuses the page, so the app's webview gets no key events
  // while browsing (`app/src-tauri/src/shell/menu.rs`). ⌘R/⌘[/⌘] are routed by the
  // shell and tab strip.
  useTauriEvent("menu-address", () => focused && addressRef.current?.focus());

  const barButton =
    "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

  const zoom = tab?.zoom ?? 1;

  return (
    <div ref={rootRef} className="flex h-full flex-col">
      <div className="relative shrink-0 border-b border-border">
        <div className="flex h-10 items-center gap-1 px-2">
          <button
            onClick={() => tab && browser.history(tab.id, -1)}
            disabled={!tab?.can_back}
            aria-label="Go back"
            className={barButton}
          >
            <CaretLeft size={15} />
          </button>
          <button
            onClick={() => tab && browser.history(tab.id, 1)}
            disabled={!tab?.can_forward}
            aria-label="Go forward"
            className={barButton}
          >
            <CaretRight size={15} />
          </button>
          <button
            onClick={() => tab && browser.reload(tab.id)}
            disabled={!tab}
            aria-label="Reload"
            className={cn(barButton, "mr-1")}
          >
            <ArrowClockwise
              size={15}
              className={cn(tab?.loading && "animate-spin")}
            />
          </button>
          <input
            ref={addressRef}
            value={address}
            disabled={!tab}
            onChange={(e) => setAddress(e.target.value)}
            onFocus={(e) => {
              setEditing(true);
              e.currentTarget.select();
              captureStill();
            }}
            onBlur={() => {
              setEditing(false);
              setAddress(tab?.url ?? "");
              setMatches([]);
            }}
            onKeyDown={onAddressKeyDown}
            spellCheck={false}
            autoComplete="off"
            className="h-7 min-w-0 flex-1 rounded-full bg-secondary px-3.5 text-[12.5px] text-foreground outline-none placeholder:text-muted-foreground focus:bg-card focus:ring-2 focus:ring-brand/40 disabled:opacity-50"
            placeholder="Search or enter address"
          />
          {/* Shown only off 100%; resets to actual size. */}
          {Math.abs(zoom - 1) > 0.001 && (
            <button
              onClick={() => tab && browser.setZoom(tab.id, 1)}
              aria-label="Reset zoom to 100%"
              title="Reset zoom"
              className="ml-1 flex h-7 shrink-0 items-center rounded-full bg-secondary px-2.5 text-[11.5px] tabular-nums text-muted-foreground transition-colors hover:text-foreground"
            >
              {Math.round(zoom * 100)}%
            </button>
          )}
          <button
            onClick={openFind}
            disabled={!tab}
            aria-label="Find in page"
            className={cn(barButton, "ml-1")}
          >
            <MagnifyingGlass size={15} />
          </button>
          <button
            onClick={() => tab && browser.external(tab.url)}
            disabled={!tab}
            aria-label="Open in default browser"
            className={barButton}
          >
            <ArrowSquareOut size={15} />
          </button>
        </div>

        {/* A toolbar row, not a floating strip: the DOM cannot draw over the
            native page. */}
        {find.open && (
          <FindBar
            inputRef={findRef}
            query={find.query}
            onQueryChange={(query) => {
              setFind((f) => ({ ...f, query, found: true }));
              runFind(query, false, true);
            }}
            onStep={(backwards) => runFind(find.query, backwards, false)}
            onClose={closeFind}
            status={find.found ? undefined : "No results"}
            placeholder="Find in page"
            className="border-t border-border-subtle"
          />
        )}

        {/* Drawn over the still. `onMouseDown`, not `onClick`: the field's
            blur would close the list before a click landed. */}
        {listOpen && (
          <div className="absolute inset-x-2 top-full z-20 mt-1 overflow-hidden rounded-lg border border-border bg-popover py-1 shadow-lg">
            {suggestions.map((s, i) => {
              const icon = faviconFor(s.url, favicons);
              return (
                <button
                  key={s.key}
                  type="button"
                  onMouseDown={(e) => {
                    e.preventDefault();
                    goTo(s.url);
                  }}
                  onMouseEnter={() => setPicked(i)}
                  className={cn(
                    "flex w-full items-center gap-2.5 px-3 py-1.5 text-left",
                    i === picked && "bg-sidebar-item-hover",
                  )}
                >
                  <span className="flex h-3.5 w-3.5 shrink-0 items-center justify-center text-muted-foreground">
                    {s.kind === "search" ? (
                      <MagnifyingGlass size={13} />
                    ) : icon ? (
                      <img
                        src={icon}
                        alt=""
                        className="h-3.5 w-3.5 rounded-[2px] object-contain"
                      />
                    ) : (
                      <Globe size={13} />
                    )}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[12px] text-foreground">
                      {s.label}
                    </span>
                    <span className="block truncate text-[11px] text-muted-foreground">
                      {s.detail}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
        )}
      </div>

      {/* The native view covers this slot; only the still renders here. */}
      <div ref={slotRef} className="relative min-h-0 flex-1">
        {(listOpen || standing) && typeof still === "string" && (
          <img
            src={still}
            alt=""
            draggable={false}
            className="pointer-events-none absolute inset-0 h-full w-full select-none object-cover object-left-top"
          />
        )}
      </div>
    </div>
  );
}
