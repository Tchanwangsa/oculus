import {
  EditorSelection,
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
  type Range,
  type Text,
  type TransactionSpec,
} from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";

/**
 * Find and replace over a note's text, for a bar drawn by the page (no UI
 * here). The state holds the query and the current match's range, mapped
 * through edits and dropped once its text stops matching; the matches
 * themselves are computed on demand and cached per document and query.
 * Every match is marked `cm-find-match`, the current one `cm-find-current`.
 *
 * A step selects the match. Live mode reveals what a selection touches only
 * while the editor has focus, and the bar holds focus, so `livePreview.ts`
 * also reveals the construct around a match the find selected
 * (`findRevealed`). Replacing goes through the note's history.
 */

/** Matches counted and drawn; `findStatus` reports more as capped. */
export const FIND_CAP = 1000;

interface Span {
  from: number;
  to: number;
}

interface FindState {
  query: string;
  caseSensitive: boolean;
  /** The match the find last selected, or null. */
  current: Span | null;
}

export interface FindStatus {
  total: number;
  /** 1-based index of the current match, 0 for none. */
  current: number;
  /** More than `total` matches exist. */
  capped: boolean;
}

const setFind = StateEffect.define<{ query: string; caseSensitive: boolean }>();
const setCurrent = StateEffect.define<Span | null>();

const EMPTY: FindState = { query: "", caseSensitive: false, current: null };

const patterns = new Map<string, RegExp>();

/** The query as a literal pattern; `^…$` when `whole` for testing one slice. */
function pattern(query: string, caseSensitive: boolean, whole = false): RegExp {
  const key = `${caseSensitive ? "C" : "I"}${whole ? "W" : "G"}${query}`;
  let re = patterns.get(key);
  if (!re) {
    const body = query.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
    re = new RegExp(whole ? `^${body}$` : body, (caseSensitive ? "u" : "iu") + (whole ? "" : "g"));
    if (patterns.size > 50) patterns.clear();
    patterns.set(key, re);
  }
  return re;
}

function matchesAt(doc: Text, f: FindState, span: Span): boolean {
  return span.to <= doc.length && pattern(f.query, f.caseSensitive, true).test(doc.sliceString(span.from, span.to));
}

const findField = StateField.define<FindState>({
  create: () => EMPTY,
  update(f, tr) {
    let next = f;
    if (tr.docChanged && next.current) {
      const from = tr.changes.mapPos(next.current.from, 1);
      const to = tr.changes.mapPos(next.current.to, -1);
      const span = { from, to };
      next = { ...next, current: from < to && matchesAt(tr.state.doc, next, span) ? span : null };
    }
    for (const e of tr.effects) {
      if (e.is(setFind)) next = { ...e.value, current: null };
      else if (e.is(setCurrent)) next = { ...next, current: e.value };
    }
    return next;
  },
});

interface Matches {
  spans: Span[];
  capped: boolean;
}

const NONE: Matches = { spans: [], capped: false };
const matchCache = new WeakMap<Text, Map<string, Matches>>();

function scan(doc: Text, query: string, caseSensitive: boolean, cap: number): Matches {
  const re = pattern(query, caseSensitive);
  const text = doc.toString();
  const spans: Span[] = [];
  re.lastIndex = 0;
  for (let m = re.exec(text); m; m = re.exec(text)) {
    if (spans.length === cap) return { spans, capped: true };
    spans.push({ from: m.index, to: m.index + m[0].length });
  }
  return { spans, capped: false };
}

/** The first `FIND_CAP` matches of `query` in `doc`, in document order. */
function cachedMatches(doc: Text, query: string, caseSensitive: boolean): Matches {
  if (!query) return NONE;
  let byQuery = matchCache.get(doc);
  if (!byQuery) matchCache.set(doc, (byQuery = new Map()));
  const key = `${caseSensitive ? "C" : "I"}${query}`;
  let found = byQuery.get(key);
  if (!found) {
    found = scan(doc, query, caseSensitive, FIND_CAP);
    byQuery.set(key, found);
  }
  return found;
}

function matchesOf(state: EditorState): Matches {
  const f = state.field(findField, false);
  return f ? cachedMatches(state.doc, f.query, f.caseSensitive) : NONE;
}

/** Index of the first span with `from >= pos`, `spans.length` if none. */
function firstFrom(spans: Span[], pos: number): number {
  let lo = 0;
  let hi = spans.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (spans[mid].from < pos) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

function indexOf(spans: Span[], span: Span | null): number {
  if (!span) return -1;
  const i = firstFrom(spans, span.from);
  return i < spans.length && spans[i].from === span.from && spans[i].to === span.to ? i : -1;
}

/** Select `span`, make it current and scroll it to the middle. */
function selectSpec(span: Span, before: StateEffect<unknown>[] = []): TransactionSpec {
  return {
    selection: EditorSelection.single(span.from, span.to),
    effects: [
      ...before,
      setCurrent.of(span),
      EditorView.scrollIntoView(EditorSelection.range(span.from, span.to), { y: "center" }),
    ],
    userEvent: "select.search",
  };
}

/** The match after `pos` (from `pos` on), or before it (ending by `pos`), wrapping. */
function stepFrom(spans: Span[], pos: number, backwards: boolean): Span | null {
  if (!spans.length) return null;
  const i = firstFrom(spans, pos);
  if (!backwards) return spans[i] ?? spans[0];
  let j = i - 1;
  while (j >= 0 && spans[j].to > pos) j--;
  return spans[j] ?? spans[spans.length - 1];
}

// ── Commands ───────────────────────────────────────────────────────────────

/** Search for `query` (literal, case-insensitive unless asked) and select the
 *  first match from the selection's start, as typing in the bar does. An
 *  empty query clears the find. */
export function setFindQuery(view: EditorView, query: string, opts: { caseSensitive?: boolean } = {}): void {
  const caseSensitive = opts.caseSensitive ?? false;
  const prev = view.state.field(findField, false);
  if (!prev) return;
  if (prev.query === query && prev.caseSensitive === caseSensitive) return;
  const { state } = view;
  const next = stepFrom(cachedMatches(state.doc, query, caseSensitive).spans, state.selection.main.from, false);
  const reset = setFind.of({ query, caseSensitive });
  view.dispatch(next ? selectSpec(next, [reset]) : { effects: reset });
}

/** Select the next match after the selection, or the previous one before it,
 *  wrapping at either end. False when nothing matches. */
export function findStep(view: EditorView, backwards: boolean): boolean {
  const { main } = view.state.selection;
  const next = stepFrom(matchesOf(view.state).spans, backwards ? main.from : main.to, backwards);
  if (!next) return false;
  view.dispatch(selectSpec(next));
  return true;
}

/** Replace the selection when it is exactly a match, then select the next
 *  one; a selection elsewhere only moves to the next match. False when
 *  nothing matches or the note is read-only. */
export function replaceCurrent(view: EditorView, replacement: string): boolean {
  const { state } = view;
  const f = state.field(findField, false);
  if (!f?.query || state.readOnly) return false;
  const { main } = state.selection;
  const replacing = state.selection.ranges.length === 1 && !main.empty && matchesAt(state.doc, f, main);
  if (replacing) {
    view.dispatch({
      changes: { from: main.from, to: main.to, insert: replacement },
      selection: EditorSelection.cursor(main.from + replacement.length),
      effects: setCurrent.of(null),
      userEvent: "input.replace",
    });
  }
  return findStep(view, false) || replacing;
}

/** Replace every match in one transaction, so one undo restores them all.
 *  Not limited to `FIND_CAP`. Returns how many were replaced. */
export function replaceAll(view: EditorView, replacement: string): number {
  const { state } = view;
  const f = state.field(findField, false);
  if (!f?.query || state.readOnly) return 0;
  const { spans } = scan(state.doc, f.query, f.caseSensitive, Infinity);
  if (!spans.length) return 0;
  view.dispatch({
    changes: spans.map((s) => ({ ...s, insert: replacement })),
    effects: setCurrent.of(null),
    userEvent: "input.replace.all",
  });
  return spans.length;
}

/** Drop the query and its highlights; the selection stays. */
export function clearFind(view: EditorView): void {
  const f = view.state.field(findField, false);
  if (f && (f.query || f.current)) view.dispatch({ effects: setFind.of({ query: "", caseSensitive: f.caseSensitive }) });
}

// ── Reading ────────────────────────────────────────────────────────────────

/** The selection's text when it is one non-empty line, to seed the bar. */
export function selectionQuery(state: EditorState): string {
  const { main } = state.selection;
  if (main.empty) return "";
  const text = state.sliceDoc(main.from, main.to);
  return text.includes("\n") ? "" : text;
}

const sameStatus = (a: FindStatus, b: FindStatus) =>
  a.total === b.total && a.current === b.current && a.capped === b.capped;

const statusCache = new WeakMap<FindState, { doc: Text; status: FindStatus }>();
const NO_STATUS: FindStatus = { total: 0, current: 0, capped: false };

/** Match count and position. The same object comes back until either
 *  changes, so it can serve as a `useSyncExternalStore` snapshot. */
export function findStatus(state: EditorState): FindStatus {
  const f = state.field(findField, false);
  if (!f?.query) return NO_STATUS;
  const cached = statusCache.get(f);
  if (cached?.doc === state.doc) return cached.status;
  const { spans, capped } = matchesOf(state);
  const status = { total: spans.length, current: indexOf(spans, f.current) + 1, capped };
  const result = cached && sameStatus(cached.status, status) ? cached.status : status;
  statusCache.set(f, { doc: state.doc, status: result });
  return result;
}

/** The selection is the match the find selected, so Live mode shows the
 *  source around it even while the bar, not the editor, has focus. */
export function findRevealed(state: EditorState): boolean {
  const cur = state.field(findField, false)?.current;
  if (!cur) return false;
  const { main, ranges } = state.selection;
  return ranges.length === 1 && main.from === cur.from && main.to === cur.to;
}

const listeners = new WeakMap<EditorView, Set<(status: FindStatus) => void>>();

/** Call `listener` whenever `view`'s `findStatus` changes. Returns the
 *  unsubscribe. */
export function subscribeFind(view: EditorView, listener: (status: FindStatus) => void): () => void {
  let set = listeners.get(view);
  if (!set) listeners.set(view, (set = new Set()));
  set.add(listener);
  return () => {
    set.delete(listener);
  };
}

// ── Drawing ────────────────────────────────────────────────────────────────

const matchMark = Decoration.mark({ class: "cm-find-match" });
const currentMark = Decoration.mark({ class: "cm-find-match cm-find-current" });

function drawMatches(view: EditorView): DecorationSet {
  const { spans } = matchesOf(view.state);
  if (!spans.length) return Decoration.none;
  const cur = view.state.field(findField).current;
  const out: Range<Decoration>[] = [];
  for (const { from, to } of view.visibleRanges) {
    for (let i = firstFrom(spans, from); i < spans.length && spans[i].from < to; i++) {
      const s = spans[i];
      out.push((cur && s.from === cur.from && s.to === cur.to ? currentMark : matchMark).range(s.from, s.to));
    }
  }
  return Decoration.set(out);
}

const findPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = drawMatches(view);
    }
    update(u: ViewUpdate) {
      const changed = u.state.field(findField) !== u.startState.field(findField);
      if (changed || u.docChanged || u.viewportChanged) this.decorations = drawMatches(u.view);
      const set = listeners.get(u.view);
      if (set?.size && (changed || u.docChanged)) {
        const status = findStatus(u.state);
        if (!sameStatus(status, findStatus(u.startState))) for (const l of [...set]) l(status);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

const brand = "var(--color-brand)";

const findTheme = EditorView.theme({
  ".cm-find-match": { backgroundColor: `color-mix(in srgb, ${brand} 18%, transparent)`, borderRadius: "2px" },
  ".cm-find-match.cm-find-current": { backgroundColor: `color-mix(in srgb, ${brand} 42%, transparent)` },
});

/** The find state, its highlights and their colours. */
export function findExtension(): Extension {
  return [findField, findPlugin, findTheme];
}
