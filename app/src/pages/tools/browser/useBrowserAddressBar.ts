import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  addressKind,
  browser,
  hostOf,
  normalizeAddress,
  searchEngine,
  type BrowserTab,
} from "@/lib/browser";
import { suggestHistory, type HistoryEntry } from "@/lib/browser/history";

/** The typed row (always first, selected by default), then history matches. */
interface Suggestion {
  key: string;
  url: string;
  label: string;
  detail: string;
  kind: "typed" | "search" | "history";
}

/**
 * The address field and its suggestion list. `still` is the page snapshot the
 * list is drawn over; the list waits on it so the page is never hidden with
 * nothing behind it.
 */
export function useBrowserAddressBar(
  tab: BrowserTab | undefined,
  id: number,
  still: string | null | undefined,
) {
  const addressRef = useRef<HTMLInputElement>(null);
  const [address, setAddress] = useState(tab?.url ?? "");
  const [editing, setEditing] = useState(false);
  const [matches, setMatches] = useState<HistoryEntry[]>([]);
  const [picked, setPicked] = useState(0);

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

  // The page stays parked behind its still for as long as the field is being
  // edited, not just while the list shows: `browser_place` focuses the native
  // page, so re-placing it when the list closes (a backspace to empty) would
  // steal the caret from the field.
  const holding = editing && typeof still === "string";

  useEffect(() => {
    if (!editing) setAddress(tab?.url ?? "");
  }, [editing, tab?.url, id]);

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

  return {
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
  };
}
