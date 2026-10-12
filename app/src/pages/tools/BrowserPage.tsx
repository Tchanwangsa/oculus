import { useRef } from "react";
import { useParams } from "react-router-dom";
import {
  ArrowClockwise,
  ArrowSquareOut,
  CaretLeft,
  CaretRight,
  MagnifyingGlass,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { browser } from "@/lib/browser";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { useBrowserStore } from "@/stores/shell/browserStore";
import { useActivePaneId } from "@/stores/shell/tabStore";
import { useTabActive, useTabId } from "@/components/tabs/TabContext";
import { FindBar } from "@/components/ui/search/FindBar";
import { AddressSuggestions } from "./browser/AddressSuggestions";
import { useBrowserAddressBar } from "./browser/useBrowserAddressBar";
import { useBrowserFind } from "./browser/useBrowserFind";
import { useBrowserViewport } from "./browser/useBrowserViewport";

/**
 * The `/browse/:id` route: a toolbar over an empty slot. Rust places the tab's
 * native page there (`app/src-tauri/src/shell/browser/`), and the app opens a
 * hole over the slot to show it (`lib/shell/nativeHoles.ts`): under the app's
 * transparent webview on macOS, clipped to the hole on Windows. Portals, the
 * suggestion list among them, draw over the live page; anything inside
 * `#root` over the slot is hidden by it.
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
  const rootRef = useRef<HTMLDivElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);

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
    goTo,
    onAddressKeyDown,
  } = useBrowserAddressBar(tab, id);
  const slotRef = useBrowserViewport(id, active);
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
      <div ref={toolbarRef} className="shrink-0 border-b border-border">
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

        {/* A toolbar row, not a floating strip: inside `#root`, a strip over
            the slot would fall in the page's hole. */}
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

        {listOpen && active && (
          <AddressSuggestions
            anchorRef={toolbarRef}
            suggestions={suggestions}
            picked={picked}
            setPicked={setPicked}
            goTo={goTo}
          />
        )}
      </div>

      {/* Empty: the native page shows through the hole cut here. */}
      <div ref={slotRef} className="min-h-0 flex-1" />
    </div>
  );
}
