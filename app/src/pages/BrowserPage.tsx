import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
import {
  addressKind,
  browser,
  hostOf,
  normalizeAddress,
  searchEngine,
  type FindResult,
  type Viewport,
} from "@/lib/browser";
import { suggestHistory, type HistoryEntry } from "@/lib/browserHistory";
import { faviconFor } from "@/hooks/useBrowserTabs";
import { useTauriEvent } from "@/hooks/useEvents";
import { useBrowserStore } from "@/stores/browserStore";
import { useActivePaneId } from "@/stores/tabStore";
import { useTabActive, useTabId } from "@/components/tabs/TabContext";
import { FindBar } from "@/components/ui/FindBar";
import { useFindTarget } from "@/lib/find";

/**
 * The `/browse/:id` route: a toolbar over an empty slot that Rust parks the
 * tab's native WKWebView over (`app/src-tauri/src/browser.rs`).
 *
 * A native view cannot sit under the DOM, so anything drawn over the slot must
 * hide the page: a portalled popover, the suggestion list, the tab going to the
 * background. They fold into one `hidden` — two effects disagreeing about one
 * page leave it parked over the app. The suggestion list is drawn over a PNG
 * still of the page (`browser_snapshot`) so the card does not blank.
 */

function appZoom(): number {
  const z = parseFloat(
    document.documentElement.style.getPropertyValue("--app-zoom"),
  );
  return Number.isFinite(z) && z > 0 ? z : 1;
}

/** The slot as window insets in logical points (CSS px × page zoom), plus the
 *  card's inner radius for the page's bottom corners. */
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

/** Whether a portal (popover, menu, dialog…) lands over the slot. A portal's
 *  wrapper is an unstyled div, so its children are measured too. */
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

/** The typed row (always first, selected by default), then history matches. */
interface Suggestion {
  key: string;
  url: string;
  label: string;
  detail: string;
  kind: "typed" | "search" | "history";
}

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
  const slotRef = useRef<HTMLDivElement>(null);
  const addressRef = useRef<HTMLInputElement>(null);
  const findRef = useRef<HTMLInputElement>(null);
  const [address, setAddress] = useState(tab?.url ?? "");
  const [editing, setEditing] = useState(false);
  const [covered, setCovered] = useState(false);
  // Blob URL once Rust answers, `null` if it cannot, `undefined` while
  // pending — which also holds the list closed.
  const [still, setStill] = useState<string | null | undefined>(undefined);
  const [standing, setStanding] = useState(false);
  const stillSeq = useRef(0);

  const [matches, setMatches] = useState<HistoryEntry[]>([]);
  const [picked, setPicked] = useState(0);
  const [find, setFind] = useState<{ open: boolean; query: string; found: boolean }>(
    { open: false, query: "", found: true },
  );

  // Typing opens the list, focusing does not — or ⌘L would hide the page.
  const draft = address.trim();
  const suggesting = editing && draft !== "" && draft !== tab?.url;

  const suggestions = useMemo<Suggestion[]>(() => {
    if (!suggesting) return [];
    const target = normalizeAddress(draft);
    const typed: Suggestion =
      addressKind(draft) === "url"
        ? {
            key: "typed",
            url: target,
            label: target,
            detail: hostOf(target),
            kind: "typed",
          }
        : {
            key: "typed",
            url: target,
            label: draft,
            detail: `Search ${searchEngine().label}`,
            kind: "search",
          };
    const rest = matches
      .filter((m) => m.url !== target)
      .map<Suggestion>((m) => ({
        key: m.url,
        url: m.url,
        label: m.title || m.url,
        detail: m.url,
        kind: "history",
      }));
    return [typed, ...rest];
  }, [suggesting, draft, matches]);

  // The list waits on the still so the page is never hidden with nothing
  // behind the list; `null` counts as settled.
  const listOpen = suggesting && suggestions.length > 0 && still !== undefined;

  // One rule, three reasons — see the file header.
  const hidden = !active || covered || listOpen;

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

  useEffect(() => {
    if (!editing) setAddress(tab?.url ?? "");
  }, [editing, tab?.url, id]);

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
  // `standing` keeps it up after the list closes, until the live page is back.
  useEffect(() => {
    if (listOpen && typeof still === "string") setStanding(true);
  }, [listOpen, still]);

  // Autocomplete: a local query per keystroke, undebounced, with an ordering
  // guard against out-of-order answers.
  const query = useRef(0);
  useEffect(() => {
    if (!suggesting) {
      setMatches([]);
      return;
    }
    const seq = ++query.current;
    suggestHistory(draft)
      .then((rows) => {
        if (query.current === seq) setMatches(rows);
      })
      .catch(() => {});
  }, [suggesting, draft]);

  // A fresh keystroke re-selects the typed row.
  useEffect(() => setPicked(0), [draft]);

  const goTo = useCallback(
    (url: string) => {
      if (!tab || !url) return;
      // Blur first: the blur handler resets the field to the old URL.
      addressRef.current?.blur();
      setEditing(false);
      setAddress(url);
      setMatches([]);
      browser.navigate(tab.id, url).catch(() => {});
    },
    [tab],
  );

  const onAddressKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      goTo(suggestions[picked]?.url ?? normalizeAddress(address));
      return;
    }
    if (e.key === "Escape") {
      e.stopPropagation();
      setEditing(false);
      setAddress(tab?.url ?? "");
      setMatches([]);
      e.currentTarget.blur();
      return;
    }
    if (!listOpen) return;
    // Tab takes the highlighted completion into the field without going.
    if (e.key === "Tab") {
      e.preventDefault();
      const suggestion = suggestions[picked];
      if (suggestion) setAddress(suggestion.url);
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setPicked((i) => (i + 1) % suggestions.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setPicked((i) => (i - 1 + suggestions.length) % suggestions.length);
    }
  };

  // Find in page: WebKit's find via Rust. It reports only matched/not (no
  // count), and searches from the current selection — so a query edit clears
  // the selection first to search again from the top.
  const runFind = useCallback(
    (text: string, backwards: boolean, fromTop: boolean) => {
      if (!tab) return;
      if (!text) {
        browser.findClear(tab.id).catch(() => {});
        setFind((f) => ({ ...f, found: true }));
        return;
      }
      const search = () => browser.find(tab.id, text, backwards).catch(() => {});
      if (fromTop) browser.findClear(tab.id).then(search).catch(search);
      else search();
    },
    [tab],
  );

  useTauriEvent<FindResult>("browser-find", (e) => {
    if (e.payload.id !== id) return;
    setFind((f) =>
      e.payload.query === f.query ? { ...f, found: e.payload.found } : f,
    );
  });

  const closeFind = useCallback(() => {
    setFind({ open: false, query: "", found: true });
    if (tab) browser.findClear(tab.id).catch(() => {});
  }, [tab]);

  // ⌘F / ⌘G / ⇧⌘G are menu events, routed by `lib/find.ts`.
  const openFind = () => {
    setFind((f) => ({ ...f, open: true }));
    // After mount; select so a second ⌘F replaces the query.
    requestAnimationFrame(() => findRef.current?.select());
  };
  const stepFind = (backwards: boolean) => {
    if (!find.open) openFind();
    else if (find.query) runFind(find.query, backwards, false);
  };
  // The pane's page-level target, so ⌘F reaches it while the native page
  // holds the keyboard (`lib/find.ts`).
  useFindTarget(rootRef, { open: openFind, step: stepFind }, paneId);

  // ⌘L. Every browser shortcut is a menu item, never a `keydown`:
  // `browser_place` focuses the page, so the app's webview gets no key events
  // while browsing (`app/src-tauri/src/menu.rs`). ⌘R/⌘[/⌘] are routed by the
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
