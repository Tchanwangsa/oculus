import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowClockwise,
  ArrowSquareOut,
  CaretDown,
  CaretLeft,
  CaretRight,
  CaretUp,
  Globe,
  MagnifyingGlass,
  X,
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
import { useBrowserStore } from "@/stores/browserStore";
import { useActivePaneId } from "@/stores/tabStore";
import { useTabActive, useTabId } from "@/components/tabs/TabContext";

/**
 * The `/browse/:id` route: a toolbar across the top of the content card and,
 * under it, an empty slot that the tab's native page WebView sits in. The page
 * is not in the DOM — Rust parks a WKWebView over the slot
 * (`app/src-tauri/src/browser.rs`) — so this component's job is to say where
 * the slot is, and to step the page aside when the app has to draw over it.
 *
 * Everything shown here comes from Rust's snapshot; the local state is the
 * address bar's draft while it is being typed in, the suggestions under it,
 * and the find bar.
 *
 * **Three things in this file hide the page, and they are one rule.** A native
 * view cannot interleave with the DOM, so anything the app draws over the slot
 * would render *beneath* the page: a portalled popover (`coveredBy`), the
 * address bar's own suggestion list, and the tab going to the background. They
 * are folded into a single `hidden` below rather than three competing calls,
 * because Rust does what it is told page by page and two effects disagreeing
 * about one page is how a page ends up parked over the app.
 *
 * The suggestion list is the only one of the three that is a *choice*, and it
 * is the only one with a **still** behind it. Taking the page away to make
 * room for a dropdown blanked the card, which is not what an omnibox does — so
 * focusing the address bar asks Rust for a PNG of the page as it stands
 * (`browser_snapshot`), the slot paints that image, and the popover is drawn
 * over the image in the ordinary way, with its shadow and its hover states.
 * What you were reading stays on screen, frozen, for the second you spend
 * typing. If no still arrives the list opens anyway and the page still goes:
 * that is the old behaviour, kept as the floor rather than as the plan.
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

/** What the address bar offers: the thing you typed, then places you have
 *  been. The typed row is always first and always selected to begin with, so
 *  Enter means what it has always meant and the list only ever adds. */
interface Suggestion {
  key: string;
  url: string;
  /** The line in bold-ish: a page's title, or the address itself. */
  label: string;
  /** The quiet line under it. */
  detail: string;
  kind: "typed" | "search" | "history";
}

export default function BrowserPage() {
  const params = useParams();
  const id = Number(params.id);
  const active = useTabActive();
  // ⌘F belongs to one page, and a split tab has two of them in front at once.
  // `active` is true for both halves — it means "this tab is the tab in
  // front" — so the find bar asks the narrower question the shell already
  // answers for navigation: which half am I working in.
  const paneId = useTabId();
  const focusedPaneId = useActivePaneId();
  const focused = active && paneId === focusedPaneId;
  const tab = useBrowserStore((s) => s.tabs.find((t) => t.id === id));
  const favicons = useBrowserStore((s) => s.favicons);
  const slotRef = useRef<HTMLDivElement>(null);
  const addressRef = useRef<HTMLInputElement>(null);
  const findRef = useRef<HTMLInputElement>(null);
  const [address, setAddress] = useState(tab?.url ?? "");
  // State, not a ref: whether the field is being typed in decides whether the
  // suggestion list is up, and the list is rendered.
  const [editing, setEditing] = useState(false);
  const [covered, setCovered] = useState(false);
  // The still of the page the suggestion list is drawn over: a blob URL once
  // Rust has answered, `null` when it cannot, `undefined` while the answer is
  // outstanding — which is also "the list is not allowed up yet", so that
  // typing never shows a dropdown over a card with nothing behind it.
  const [still, setStill] = useState<string | null | undefined>(undefined);
  const [standing, setStanding] = useState(false);
  const stillSeq = useRef(0);

  const [matches, setMatches] = useState<HistoryEntry[]>([]);
  const [picked, setPicked] = useState(0);
  const [find, setFind] = useState<{ open: boolean; query: string; found: boolean }>(
    { open: false, query: "", found: true },
  );

  // Typing something other than where you already are is what opens the list;
  // focusing the field is not, or ⌘L would blank the page you are reading
  // before you have asked for anything.
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

  // The list waits on the snapshot, not the other way round: opening it first
  // would hide the page for the 30ms the still takes, which is the flash this
  // whole mechanism exists to remove. `null` — no still to be had — counts as
  // settled, and the page simply goes, as it always used to.
  const listOpen = suggesting && suggestions.length > 0 && still !== undefined;

  // ── The page's slot ────────────────────────────────────────────────────
  //
  // One rule, three reasons. See the note at the top of the file.
  const hidden = !active || covered || listOpen;

  useEffect(() => {
    const slot = slotRef.current;
    if (!slot || !Number.isInteger(id)) return;
    if (hidden) {
      browser.hideTab(id).catch(() => {});
      return;
    }
    // The still is dropped when Rust says the live page is up again, not when
    // the list closes: between those two is a frame with neither, and that
    // frame is a flash of empty card.
    browser
      .place(id, measure(slot))
      .catch(() => {})
      .finally(() => {
        setStanding(false);
        setStill(undefined);
      });
  }, [id, hidden]);

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

  // Watch for portals landing over the slot. Measured a frame after the
  // mutation, once the popper has positioned itself. Only portals: this
  // component's own overlays are inside `#root` and are accounted for above,
  // so opening the suggestion list cannot feed back into this.
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

  // The address bar shows the tab's URL unless it is being typed in.
  useEffect(() => {
    if (!editing) setAddress(tab?.url ?? "");
  }, [editing, tab?.url, id]);

  // ── The still ─────────────────────────────────────────────────────────
  //
  // Taken when the field is focused rather than when the list opens: a
  // snapshot is a round trip through WebKit, and focus is the keystroke
  // before the one that needs it. Sequenced like the autocomplete below,
  // because a snapshot that lands after you have already left is a picture of
  // a page you are no longer looking at.
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

  // A blob URL is a live allocation — a window-sized 2x PNG of it — so the
  // state that holds one owns it: whenever `still` moves on, the URL it held
  // goes back to the browser.
  useEffect(() => {
    if (typeof still !== "string") return;
    return () => URL.revokeObjectURL(still);
  }, [still]);

  // `listOpen` puts the still in the slot during the *render* that opens the
  // list, before the effect above tells Rust to take the page down — the image
  // is under the page until then, so it costs nothing to have it early and it
  // means there is never a frame with neither. This flag is only what keeps it
  // there afterwards, until the live page is back.
  useEffect(() => {
    if (listOpen && typeof still === "string") setStanding(true);
  }, [listOpen, still]);

  // ── Autocomplete ──────────────────────────────────────────────────────
  //
  // A local SQLite query per keystroke, which is cheap enough not to debounce;
  // what it does need is an ordering guard, since two queries in flight can
  // land out of order and leave the list showing answers to an older prefix.
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

  // The typed row is first, so a fresh keystroke always re-selects it: what
  // you are typing must never be overtaken by a suggestion that happened to
  // stay in the list.
  useEffect(() => setPicked(0), [draft]);

  const goTo = useCallback(
    (url: string) => {
      if (!tab || !url) return;
      // Blur *first*: the blur handler puts the field back to the tab's
      // current URL, and doing it the other way round would land that stale
      // value on top of the address we are on our way to.
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
    // Tab fills the field with the highlighted row without going there — the
    // way an address bar lets you take a completion and then edit it.
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

  // ── Find in page ──────────────────────────────────────────────────────
  //
  // WebKit's own find, driven from Rust (`browser.rs`). Two things follow from
  // that API: there is no match *count*, only whether anything matched, so the
  // bar says "No results" and never "3 of 12"; and each search starts from the
  // current selection, so an edit to the query clears the selection first and
  // searches again from the top — which is what makes typing feel incremental
  // rather than walking forward a match per keystroke.
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

  useEffect(() => {
    const unlisten = listen<FindResult>("browser-find", (e) => {
      if (e.payload.id !== id) return;
      setFind((f) =>
        e.payload.query === f.query ? { ...f, found: e.payload.found } : f,
      );
    });
    return () => void unlisten.then((off) => off()).catch(() => {});
  }, [id]);

  const closeFind = useCallback(() => {
    setFind({ open: false, query: "", found: true });
    if (tab) browser.findClear(tab.id).catch(() => {});
  }, [tab]);

  // ⌘F, ⌘G and ⇧⌘G arrive as menu events, not key presses: the page is a
  // native WebView that takes every ⌘-key, so a `keydown` here would only ever
  // work from outside the page it is meant to search
  // (`app/src-tauri/src/menu.rs`).
  const findActions = useRef({ open: () => {}, step: (_: boolean) => {} });
  findActions.current = {
    open: () => {
      setFind((f) => ({ ...f, open: true }));
      // After the bar has mounted, and selecting so ⌘F twice replaces rather
      // than appends.
      requestAnimationFrame(() => findRef.current?.select());
    },
    step: (backwards: boolean) => {
      if (!find.open) {
        findActions.current.open();
        return;
      }
      if (find.query) runFind(find.query, backwards, false);
    },
  };

  useEffect(() => {
    if (!focused) return;
    const pending = [
      listen("menu-find", () => findActions.current.open()),
      listen("menu-find-next", () => findActions.current.step(false)),
      listen("menu-find-prev", () => findActions.current.step(true)),
    ];
    return () => {
      for (const p of pending) p.then((off) => off()).catch(() => {});
    };
  }, [focused]);

  // ⌘L. **Every browser shortcut here is a menu item**, none of them a
  // `keydown` listener: `browser_place` calls `set_focus()` on the page, so
  // while you are browsing the app's own webview receives no key events at all
  // — and macOS gives the menu bar first refusal on ⌘-keys even when it does.
  // A listener in this file worked only in the sliver where the app happened
  // to have focus, which is exactly not when you reach for ⌘R.
  // ⌘R, ⌘[ and ⌘] are routed by the shell and the strip, which own what they
  // mean on a page that is not a browser tab; ⌘L means nothing there at all,
  // so it ends here.
  useEffect(() => {
    if (!focused) return;
    const unlisten = listen("menu-address", () => addressRef.current?.focus());
    return () => void unlisten.then((off) => off()).catch(() => {});
  }, [focused]);

  const barButton =
    "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

  const zoom = tab?.zoom ?? 1;

  return (
    <div className="flex h-full flex-col">
      {/* The hairline under the toolbar is where the page begins. */}
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
          {/* Only when it is not 100%: a zoom control that is always there is
              a permanent reminder of a setting almost nobody changes, and
              ⌘= / ⌘− / View → Zoom are how it is changed anyway. Clicking it
              puts the page back to actual size. */}
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
            onClick={() => findActions.current.open()}
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

        {/* The find bar is a second row of the toolbar rather than a strip
            floating over the page: over the page it would be under it, since
            the page is a native view this DOM cannot draw on top of. */}
        {find.open && (
          <div className="flex h-9 items-center gap-1 border-t border-border-subtle px-2">
            <MagnifyingGlass size={13} className="mx-1.5 shrink-0 text-muted-foreground" />
            <input
              ref={findRef}
              autoFocus
              value={find.query}
              onChange={(e) => {
                const query = e.target.value;
                setFind((f) => ({ ...f, query, found: true }));
                runFind(query, false, true);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  runFind(find.query, e.shiftKey, false);
                }
                if (e.key === "Escape") {
                  e.stopPropagation();
                  closeFind();
                }
              }}
              spellCheck={false}
              placeholder="Find in page"
              className="h-7 min-w-0 flex-1 bg-transparent text-[12.5px] text-foreground outline-none placeholder:text-muted-foreground"
            />
            {/* No "3 of 12": WebKit's find API answers whether it matched and
                nothing more (see `find_string` in browser.rs). */}
            {find.query && !find.found && (
              <span className="mr-1 shrink-0 text-[11.5px] text-muted-foreground">
                No results
              </span>
            )}
            <button
              onClick={() => runFind(find.query, true, false)}
              disabled={!find.query}
              aria-label="Previous match"
              className={barButton}
            >
              <CaretUp size={13} />
            </button>
            <button
              onClick={() => runFind(find.query, false, false)}
              disabled={!find.query}
              aria-label="Next match"
              className={barButton}
            >
              <CaretDown size={13} />
            </button>
            <button onClick={closeFind} aria-label="Close find bar" className={barButton}>
              <X size={13} />
            </button>
          </div>
        )}

        {/* The omnibox dropdown, over the page — over the page's *still*,
            strictly, since the DOM cannot draw on a native view (see the note
            at the top of the file). `onMouseDown` rather than `onClick`: the
            field's blur would close this list out from under the pointer
            before a click could land. */}
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

      {/* The page's slot. Nothing renders here — the native view covers it —
          except the still that stands in for the page while the suggestion
          list is over it. */}
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
