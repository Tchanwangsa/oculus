/**
 * Inline AI suggestions ("ghost text"). `SUGGEST_DELAY_MS` after the last
 * edit, with one empty caret outside code and frontmatter and no completion
 * list open, the text around the caret goes to `SuggestConfig.fetch`; the
 * answer shows as a muted widget at the caret. Tab takes all of it, Mod-→ the
 * next word, Esc drops it. Typing what the ghost says eats it from the front;
 * any other edit, a caret move or a blur drops it.
 *
 * On and off through `suggestCompartment`, so the toggle never rebuilds the
 * view. The fetcher is a facet, so this module stays free of Tauri: the page
 * supplies `document_suggest`.
 */
import { completionStatus } from "@codemirror/autocomplete";
import { isolateHistory } from "@codemirror/commands";
import {
  Compartment,
  EditorSelection,
  Facet,
  Prec,
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
  type Transaction,
} from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  WidgetType,
  keymap,
  type ViewUpdate,
} from "@codemirror/view";
import { ancestorAt } from "./syntax";
import { SUGGEST_IDLE, type SuggestStatus } from "../DocumentControls";

/** Quiet time after the last edit before a request goes out. */
export const SUGGEST_DELAY_MS = 500;
/** Context sent either side of the caret. */
const BEFORE_CHARS = 4000;
const AFTER_CHARS = 1500;

export interface SuggestRequest {
  requestId: number;
  before: string;
  after: string;
}

export interface SuggestConfig {
  /** The text to insert at the caret; `""` for none. */
  fetch(req: SuggestRequest): Promise<string>;
  /** Drop whatever is in flight. */
  cancel(): void;
  onStatus(status: SuggestStatus): void;
}

const suggestConfig = Facet.define<SuggestConfig, SuggestConfig | null>({
  combine: (values) => values[0] ?? null,
});

export const suggestCompartment = new Compartment();

export function suggestExtension(on: boolean, config: SuggestConfig): Extension {
  return on ? aiSuggest(config) : [];
}

// ── Ghost ─────────────────────────────────────────────────────────────────

interface Ghost {
  pos: number;
  text: string;
}

const setGhost = StateEffect.define<Ghost | null>();

/** The ghost after an edit: kept, shorter, only when the edit typed its
 *  front at the caret. */
function consume(ghost: Ghost, tr: Transaction): Ghost | null {
  let changes = 0;
  let at = -1;
  let typed = "";
  tr.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    changes++;
    if (fromA === toA) {
      at = fromA;
      typed = inserted.toString();
    }
  });
  if (changes !== 1 || at !== ghost.pos || !typed || !ghost.text.startsWith(typed)) return null;
  const pos = at + typed.length;
  const sel = tr.state.selection;
  if (sel.ranges.length !== 1 || !sel.main.empty || sel.main.head !== pos) return null;
  const text = ghost.text.slice(typed.length);
  return text ? { pos, text } : null;
}

class GhostWidget extends WidgetType {
  constructor(readonly text: string) {
    super();
  }

  eq(other: GhostWidget): boolean {
    return other.text === this.text;
  }

  toDOM(): HTMLElement {
    const span = document.createElement("span");
    span.className = "cm-ghost";
    span.setAttribute("aria-hidden", "true");
    span.textContent = this.text;
    return span;
  }

  updateDOM(dom: HTMLElement): boolean {
    dom.textContent = this.text;
    return true;
  }
}

const ghostField = StateField.define<Ghost | null>({
  create: () => null,
  update(ghost, tr) {
    for (const e of tr.effects) if (e.is(setGhost)) return e.value;
    if (!ghost) return null;
    // An open completion list owns the caret's keys; the two never overlap.
    if (completionStatus(tr.state) === "active") return null;
    if (tr.docChanged) return consume(ghost, tr);
    if (tr.selection) {
      const sel = tr.state.selection;
      return sel.ranges.length === 1 && sel.main.empty && sel.main.head === ghost.pos
        ? ghost
        : null;
    }
    return ghost;
  },
  provide: (f) =>
    EditorView.decorations.from(f, (g) =>
      g
        ? Decoration.set(Decoration.widget({ widget: new GhostWidget(g.text), side: 1 }).range(g.pos))
        : Decoration.none,
    ),
});

// ── When to ask ───────────────────────────────────────────────────────────

const NO_SUGGEST = new Set(["FencedCode", "CodeBlock", "Frontmatter"]);
const WORD_CHAR = /[\p{L}\p{N}]/u;

function canSuggest(state: EditorState): boolean {
  const sel = state.selection;
  if (sel.ranges.length !== 1 || !sel.main.empty || state.doc.length === 0) return false;
  if (completionStatus(state) !== null) return false;
  const head = sel.main.head;
  // Mid-word, an insertion would split the word; not worth a turn.
  if (WORD_CHAR.test(state.sliceDoc(head, head + 1))) return false;
  return ancestorAt(state, head, (node) => NO_SUGGEST.has(node.name), [-1]) === null;
}

/** Module-wide: Rust keeps one suggestion in flight across every editor, and
 *  a higher id supersedes a lower one. */
let nextRequestId = 0;

class SuggestPlugin {
  private timer: number | null = null;
  /** The request being awaited; 0 when none. Any other answer is stale. */
  private latest = 0;
  private status: SuggestStatus = SUGGEST_IDLE;
  private config: SuggestConfig | null;

  constructor(readonly view: EditorView) {
    this.config = view.state.facet(suggestConfig);
  }

  update(u: ViewUpdate) {
    this.config = u.state.facet(suggestConfig) ?? this.config;
    if (u.focusChanged && !u.view.hasFocus) {
      this.stop();
      return;
    }
    if (u.docChanged) {
      this.clearTimer();
      // A ghost that survived the edit was typed into; it stands.
      if (!u.state.field(ghostField)) {
        this.timer = window.setTimeout(() => this.fire(), SUGGEST_DELAY_MS);
      }
    } else if (u.selectionSet) {
      this.clearTimer();
    }
  }

  destroy() {
    this.stop();
    this.config?.onStatus(SUGGEST_IDLE);
  }

  private clearTimer() {
    if (this.timer != null) window.clearTimeout(this.timer);
    this.timer = null;
  }

  private setStatus(status: SuggestStatus) {
    if (status.pending === this.status.pending && status.error === this.status.error) return;
    this.status = status;
    this.config?.onStatus(status);
  }

  /** Blur, toggle-off, note switch: nothing pending may land. */
  private stop() {
    this.clearTimer();
    if (this.latest) {
      this.latest = 0;
      this.config?.cancel();
    }
    this.setStatus({ ...this.status, pending: false });
  }

  private fire() {
    this.timer = null;
    const { view, config } = this;
    const { state } = view;
    if (!config || !view.hasFocus || state.field(ghostField) || !canSuggest(state)) return;
    const head = state.selection.main.head;
    const doc = state.doc;
    const id = ++nextRequestId;
    this.latest = id;
    this.setStatus({ ...this.status, pending: true });
    config
      .fetch({
        requestId: id,
        before: doc.sliceString(Math.max(0, head - BEFORE_CHARS), head),
        after: doc.sliceString(head, Math.min(doc.length, head + AFTER_CHARS)),
      })
      .then(
        (answer) => {
          if (id !== this.latest) return;
          this.latest = 0;
          this.setStatus({ pending: false, error: null });
          const text = answer.trimEnd();
          const now = this.view.state;
          if (!text.trim() || now.doc !== doc || now.selection.main.head !== head) return;
          if (!this.view.hasFocus || now.field(ghostField) || !canSuggest(now)) return;
          this.view.dispatch({ effects: setGhost.of({ pos: head, text }) });
        },
        (e) => {
          if (id !== this.latest) return;
          this.latest = 0;
          this.setStatus({ pending: false, error: String(e) });
        },
      );
  }
}

const suggestPlugin = ViewPlugin.fromClass(SuggestPlugin);

// ── Keys ──────────────────────────────────────────────────────────────────

/** Insert the front of the ghost that `part` picks, as its own undo step. */
function accept(view: EditorView, part: (text: string) => string): boolean {
  const ghost = view.state.field(ghostField, false);
  if (!ghost || completionStatus(view.state) === "active") return false;
  const insert = part(ghost.text);
  const rest = ghost.text.slice(insert.length);
  const end = ghost.pos + insert.length;
  view.dispatch({
    changes: { from: ghost.pos, insert },
    selection: EditorSelection.cursor(end),
    effects: setGhost.of(rest ? { pos: end, text: rest } : null),
    annotations: isolateHistory.of("full"),
    userEvent: "input.complete",
    scrollIntoView: true,
  });
  return true;
}

const nextWord = (text: string) => /^\s*\S+/.exec(text)?.[0] ?? text;

// Prec.highest: above noteKeymap's Tab (high) and the snippet-field keymap,
// also highest but appended to the config later. Every binding passes without
// a ghost or under an open completion list, so those keep their keys then.
const ghostKeymap = Prec.highest(
  keymap.of([
    { key: "Tab", run: (view) => accept(view, (text) => text) },
    { key: "Mod-ArrowRight", run: (view) => accept(view, nextWord) },
    {
      key: "Escape",
      run: (view) => {
        if (!view.state.field(ghostField, false)) return false;
        view.dispatch({ effects: setGhost.of(null) });
        return true;
      },
    },
  ]),
);

const ghostTheme = EditorView.theme({
  ".cm-ghost": {
    color: "var(--color-muted-foreground)",
    whiteSpace: "pre-wrap",
    pointerEvents: "none",
    userSelect: "none",
    WebkitUserSelect: "none",
  },
});

export function aiSuggest(config: SuggestConfig): Extension {
  return [
    suggestConfig.of(config),
    ghostField,
    suggestPlugin,
    ghostKeymap,
    ghostTheme,
    EditorView.focusChangeEffect.of((_state, focusing) => (focusing ? null : setGhost.of(null))),
  ];
}
