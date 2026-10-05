import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import type { FindBarProps } from "@/components/ui/FindBar";
import { nestedFindRoots, onScreen } from "@/lib/find";
import { buildCorpus, findMatches, locate, type TextSegment } from "@/lib/findText";

/**
 * Find in rendered DOM: plain-text, case-insensitive, painted with the CSS
 * Custom Highlight API (`::highlight(find-match)` / `find-current` in
 * `index.css`), so the page's DOM is never touched. Text that isn't drawn is
 * skipped — `[data-find-skip]` subtrees (the bars, virtualised lists), other
 * find targets' roots, collapsed `<details>`, hidden elements. While open it
 * re-searches on DOM changes under the root, so a streaming reply stays lit.
 */

/** Highlights are document-wide, so every DOM find shares one per name and
 *  adds or removes only its own ranges. */
function shared(name: string, priority = 0): Highlight | null {
  if (typeof Highlight !== "function" || !CSS.highlights) return null;
  let h = CSS.highlights.get(name);
  if (!h) {
    h = new Highlight();
    h.priority = priority;
    CSS.highlights.set(name, h);
  }
  return h;
}

const MAX_MATCHES = 5000;
const RESEARCH_MS = 150;
const SKIP =
  "script, style, noscript, template, textarea, input, select, [data-find-skip], .sr-only, .katex-mathml";

/** The root's drawn text nodes in document order, with block boundaries. */
function collect(root: HTMLElement): { nodes: Text[]; segments: TextSegment[] } {
  const skip = nestedFindRoots(root);
  const nodes: Text[] = [];
  const segments: TextSegment[] = [];
  let blocks = 0;
  let lastBlock = -1;
  let broken = false;
  const visit = (parent: Element, block: number, only?: string) => {
    for (let child = parent.firstChild; child; child = child.nextSibling) {
      if (child.nodeType === Node.TEXT_NODE) {
        const text = child.nodeValue;
        if (!text || only) continue;
        segments.push({ text, breakBefore: broken || block !== lastBlock });
        nodes.push(child as Text);
        lastBlock = block;
        broken = false;
        continue;
      }
      if (!(child instanceof Element)) continue;
      if (only && child.tagName !== only) continue;
      if (child.tagName === "BR") {
        broken = true;
        continue;
      }
      if (skip.has(child as HTMLElement) || child.matches(SKIP)) continue;
      if (child.checkVisibility?.({ visibilityProperty: true, opacityProperty: true }) === false) continue;
      const display = getComputedStyle(child).display;
      const inline = display === "inline" || display === "contents";
      const own = inline ? block : ++blocks;
      // A closed <details> draws only its summary.
      if (child instanceof HTMLDetailsElement && !child.open) visit(child, own, "SUMMARY");
      else visit(child, own);
    }
  };
  visit(root, 0);
  return { nodes, segments };
}

function scrollsOn(el: Element, axis: "x" | "y"): boolean {
  const style = getComputedStyle(el);
  const overflow = axis === "y" ? style.overflowY : style.overflowX;
  if (overflow !== "auto" && overflow !== "scroll" && overflow !== "overlay") return false;
  return axis === "y" ? el.scrollHeight > el.clientHeight : el.scrollWidth > el.clientWidth;
}

/** Centres `range` in each scrolling ancestor it sits outside of. Rects scale
 *  with page zoom and `scrollTop` doesn't, so offsets are rescaled by each
 *  scroller's rect-to-layout ratio. */
function reveal(range: Range): void {
  for (let el = range.startContainer.parentElement; el && el !== document.body; el = el.parentElement) {
    const y = scrollsOn(el, "y");
    const x = scrollsOn(el, "x");
    if (!x && !y) continue;
    const r = range.getBoundingClientRect();
    const box = el.getBoundingClientRect();
    if (y && (r.top < box.top || r.bottom > box.bottom)) {
      const scale = el.offsetHeight ? box.height / el.offsetHeight : 1;
      el.scrollTop += (r.top + r.height / 2 - (box.top + box.height / 2)) / scale;
    }
    if (x && (r.left < box.left || r.right > box.right)) {
      const scale = el.offsetWidth ? box.width / el.offsetWidth : 1;
      el.scrollLeft += (r.left + r.width / 2 - (box.left + box.width / 2)) / scale;
    }
  }
}

/** The selected text when the selection lies in `root`, whitespace-collapsed. */
function selectedIn(root: HTMLElement): string {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || !selection.rangeCount) return "";
  if (!root.contains(selection.getRangeAt(0).commonAncestorContainer)) return "";
  return selection.toString().replace(/\s+/g, " ").trim();
}

export interface DomFind {
  open: boolean;
  /** ⌘F: opens the bar, seeding it from a selection in the root, and
   *  selects the query. */
  openFind(): void;
  /** ⌘G / ⇧⌘G and the bar's arrows: wraps around; opens the bar if closed. */
  step(backwards: boolean): void;
  close(): void;
  /** Spread into `FindBar`. */
  bar: Pick<FindBarProps, "inputRef" | "query" | "onQueryChange" | "onStep" | "onClose" | "status">;
}

export function useDomFind(rootRef: RefObject<HTMLElement | null>): DomFind {
  const [open, setOpen] = useState(false);
  const [query, setQueryState] = useState("");
  const [status, setStatus] = useState<string>();
  const inputRef = useRef<HTMLInputElement>(null);
  const s = useRef({
    open: false,
    query: "",
    ranges: [] as Range[],
    current: -1,
    capped: false,
    /** Painted into the shared highlights, to take back out. */
    painted: [] as Range[],
    paintedCurrent: null as Range | null,
    /** The DOM changed while the root was hidden. */
    stale: false,
    /** Focus before the bar opened, given back on close. */
    restore: null as HTMLElement | null,
  }).current;

  const paint = useCallback(() => {
    const match = shared("find-match");
    const current = shared("find-current", 1);
    if (!match || !current) return;
    for (const r of s.painted) match.delete(r);
    if (s.paintedCurrent) current.delete(s.paintedCurrent);
    s.painted = s.ranges.filter((_, i) => i !== s.current);
    s.paintedCurrent = s.ranges[s.current] ?? null;
    for (const r of s.painted) match.add(r);
    if (s.paintedCurrent) current.add(s.paintedCurrent);
    const n = s.ranges.length;
    setStatus(
      !s.query.trim() ? undefined : n ? `${s.current + 1} of ${n}${s.capped ? "+" : ""}` : "No results",
    );
  }, [s]);

  /** Searches again. `keep` holds the current match across a DOM change;
   *  otherwise the first match not above the root's top is current. */
  const search = useCallback(
    (keep: boolean, scroll: boolean) => {
      const root = rootRef.current;
      if (!root) return;
      if (!onScreen(root)) {
        s.stale = true;
        return;
      }
      s.stale = false;
      if (!s.query.trim()) {
        s.ranges = [];
        s.current = -1;
        paint();
        return;
      }
      const previous = s.ranges[s.current];
      const { nodes, segments } = collect(root);
      const corpus = buildCorpus(segments);
      const { matches, capped } = findMatches(corpus, s.query, MAX_MATCHES);
      s.capped = capped;
      s.ranges = matches.map((m) => {
        const a = locate(corpus, m.start, false);
        const b = locate(corpus, m.end, true);
        const range = new Range();
        range.setStart(nodes[a.segment], a.offset);
        range.setEnd(nodes[b.segment], b.offset);
        return range;
      });
      // A live range follows its text through edits; take the match at or after it.
      const after = (r: Range, from: Range) =>
        r.compareBoundaryPoints(Range.START_TO_START, from) >= 0;
      let current = -1;
      if (keep && previous && previous.startContainer.isConnected) {
        current = s.ranges.findIndex((r) => after(r, previous));
      } else if (!keep) {
        const top = root.getBoundingClientRect().top;
        current = s.ranges.findIndex((r) => r.getBoundingClientRect().bottom > top);
      }
      if (current < 0) current = keep ? Math.min(Math.max(s.current, 0), s.ranges.length - 1) : 0;
      s.current = s.ranges.length ? current : -1;
      paint();
      if (scroll && s.ranges[s.current]) reveal(s.ranges[s.current]);
    },
    [rootRef, s, paint],
  );

  const setQuery = useCallback(
    (q: string) => {
      s.query = q;
      setQueryState(q);
      search(false, true);
    },
    [s, search],
  );

  const openFind = useCallback(() => {
    const root = rootRef.current;
    if (!root) return;
    if (!s.open) {
      const active = document.activeElement;
      s.restore =
        active instanceof HTMLElement && active !== document.body ? active : null;
    }
    const picked = document.activeElement === inputRef.current ? "" : selectedIn(root);
    const wasOpen = s.open;
    s.open = true;
    setOpen(true);
    if (picked && picked !== s.query) setQuery(picked);
    else if (!wasOpen && s.query) search(false, true);
    // After mount when the bar was closed; select so a repeat ⌘F replaces it.
    requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    });
  }, [rootRef, s, search, setQuery]);

  const clear = useCallback(() => {
    s.ranges = [];
    s.current = -1;
    paint();
  }, [s, paint]);

  const close = useCallback(() => {
    if (!s.open) return;
    s.open = false;
    setOpen(false);
    clear();
    const restore = s.restore;
    s.restore = null;
    if (restore?.isConnected) restore.focus({ preventScroll: true });
  }, [s, clear]);

  const step = useCallback(
    (backwards: boolean) => {
      if (!s.open) {
        openFind();
        return;
      }
      if (s.stale) search(true, false);
      const n = s.ranges.length;
      if (!n) return;
      s.current = (s.current + (backwards ? -1 : 1) + n) % n;
      paint();
      reveal(s.ranges[s.current]);
    },
    [s, openFind, search, paint],
  );

  // Re-search on DOM changes, ignoring the bar's own and nested targets'.
  useEffect(() => {
    const root = rootRef.current;
    if (!open || !root) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new MutationObserver((records) => {
      const skip = nestedFindRoots(root);
      const relevant = records.some((r) => {
        const el = r.target instanceof Element ? r.target : r.target.parentElement;
        if (!el || el.closest("[data-find-skip]")) return false;
        for (const nested of skip) if (nested.contains(el)) return false;
        return true;
      });
      if (!relevant || !s.query.trim()) return;
      clearTimeout(timer);
      timer = setTimeout(() => search(true, false), RESEARCH_MS);
    });
    observer.observe(root, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
      attributeFilter: ["open", "hidden"],
    });
    return () => {
      observer.disconnect();
      clearTimeout(timer);
    };
  }, [open, rootRef, s, search]);

  useEffect(() => clear, [clear]);

  return {
    open,
    openFind,
    step,
    close,
    bar: { inputRef, query, onQueryChange: setQuery, onStep: step, onClose: close, status },
  };
}
