import { useCallback, useEffect, useRef, useState, useSyncExternalStore, type RefObject } from "react";
import type { EditorView } from "@codemirror/view";

import type { FindBarProps } from "@/components/ui/FindBar";
import { registerFindTarget } from "@/lib/find";

import {
  clearFind,
  findStatus,
  findStep,
  replaceAll,
  replaceCurrent,
  selectionQuery,
  setFindQuery,
  subscribeFind,
  type FindStatus,
} from "./find";

export interface EditorFind {
  open: boolean;
  /** Spread into `FindBar` with a placeholder and variant. */
  bar: Omit<FindBarProps, "placeholder" | "variant" | "className">;
}

const NO_STATUS: FindStatus = { total: 0, current: 0, capped: false };
const noop = () => {};

/**
 * The find bar over a note editor's `find.ts`, shared by `DocumentEditor` and
 * `NoteField`. While `view` exists, `rootRef`'s element is a find target
 * (`lib/find.ts`) and ⌥⌘F in it opens the bar with replace unfolded. Closing
 * clears the highlights and focuses the editor on the match last selected.
 */
export function useEditorFind(
  view: EditorView | null,
  rootRef: RefObject<HTMLElement | null>,
): EditorFind {
  const [open, setOpen] = useState(false);
  const [query, setQueryState] = useState("");
  const [replacement, setReplacement] = useState("");
  const [replaceOpen, setReplaceOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const s = useRef({ open: false, query: "" }).current;

  const subscribe = useCallback(
    (cb: () => void) => (view ? subscribeFind(view, cb) : noop),
    [view],
  );
  const status = useSyncExternalStore(subscribe, () => (view ? findStatus(view.state) : NO_STATUS));

  const setQuery = useCallback(
    (q: string) => {
      s.query = q;
      setQueryState(q);
      if (view) setFindQuery(view, q);
    },
    [s, view],
  );

  /** ⌘F: seed from a one-line selection, or re-run the last query on a
   *  closed bar; then select the field so a repeat ⌘F replaces it. */
  const openFind = useCallback(
    (withReplace: boolean) => {
      if (!view) return;
      const picked = document.activeElement === inputRef.current ? "" : selectionQuery(view.state);
      const wasOpen = s.open;
      s.open = true;
      setOpen(true);
      if (withReplace) setReplaceOpen(true);
      if (picked && picked !== s.query) setQuery(picked);
      else if (!wasOpen && s.query) setFindQuery(view, s.query);
      requestAnimationFrame(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
      });
    },
    [s, view, setQuery],
  );

  const close = useCallback(() => {
    if (!s.open) return;
    s.open = false;
    setOpen(false);
    if (!view) return;
    clearFind(view);
    view.focus();
  }, [s, view]);

  const step = useCallback(
    (backwards: boolean) => {
      if (!s.open) openFind(false);
      else if (view) findStep(view, backwards);
    },
    [s, view, openFind],
  );

  const handlers = useRef({ openFind, step });
  handlers.current = { openFind, step };

  useEffect(() => {
    const root = rootRef.current;
    if (!view || !root) return;
    const unregister = registerFindTarget({
      root,
      open: () => handlers.current.openFind(false),
      step: (backwards) => handlers.current.step(backwards),
    });
    // The menu leaves ⌥⌘F to the webview; `code`, since ⌥ turns F into ƒ.
    const onKey = (e: KeyboardEvent) => {
      if (!e.metaKey || !e.altKey || e.shiftKey || e.ctrlKey || e.code !== "KeyF") return;
      e.preventDefault();
      e.stopPropagation();
      handlers.current.openFind(!view.state.readOnly);
    };
    root.addEventListener("keydown", onKey, true);
    return () => {
      unregister();
      root.removeEventListener("keydown", onKey, true);
      // The find state lives in the view, which goes with it.
      s.open = false;
      setOpen(false);
    };
  }, [view, rootRef, s]);

  const { total, current, capped } = status;
  const count = `${total}${capped ? "+" : ""}`;
  return {
    open,
    bar: {
      inputRef,
      query,
      onQueryChange: setQuery,
      onStep: step,
      onClose: close,
      status: !query
        ? undefined
        : !total
          ? "No results"
          : current
            ? `${current} of ${count}`
            : `${count} ${total === 1 ? "match" : "matches"}`,
      replace:
        !view || view.state.readOnly
          ? undefined
          : {
              value: replacement,
              onChange: setReplacement,
              onReplace: () => void replaceCurrent(view, replacement),
              onReplaceAll: () => void replaceAll(view, replacement),
              open: replaceOpen,
              onToggle: () => setReplaceOpen((o) => !o),
            },
    },
  };
}
