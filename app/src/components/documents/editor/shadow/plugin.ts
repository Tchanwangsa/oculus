import { syntaxTree, syntaxTreeAvailable } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import { ViewPlugin, type EditorView, type ViewUpdate } from "@codemirror/view";

import type { CheckOptions } from "./checks";

/** Quiet time after the last edit before the whole text and tree are compared. */
const IDLE_MS = 1000;
/** Idle rounds to wait for Lezer to cover the document before giving up
 *  until the next edit. */
const TREE_TRIES = 5;

type Cancel = () => void;

function whenIdle(fn: () => void): Cancel {
  if (typeof requestIdleCallback === "function") {
    const id = requestIdleCallback(fn, { timeout: IDLE_MS * 2 });
    return () => cancelIdleCallback(id);
  }
  const id = setTimeout(fn, 0);
  return () => clearTimeout(id);
}

/** The view-side checks, on dispatched states only: depths on every update
 *  that changes the doc or selection, and the full comparison once the
 *  editor has been idle, when Lezer's tree covers the whole document. */
export function shadowPlugin(check: (state: EditorState, opts?: CheckOptions) => void) {
  return ViewPlugin.fromClass(
    class {
      private cancel: Cancel | null = null;
      private tries = 0;

      constructor(readonly view: EditorView) {
        this.schedule();
      }

      update(u: ViewUpdate) {
        if (u.docChanged || u.selectionSet) check(u.state);
        if (u.docChanged) {
          this.tries = 0;
          this.schedule();
        }
      }

      destroy() {
        this.cancel?.();
      }

      private schedule() {
        this.cancel?.();
        const timer = setTimeout(() => {
          this.cancel = whenIdle(() => {
            this.cancel = null;
            this.full();
          });
        }, IDLE_MS);
        this.cancel = () => clearTimeout(timer);
      }

      private full() {
        const state = this.view.state;
        const len = state.doc.length;
        const tree = syntaxTreeAvailable(state, len) ? syntaxTree(state) : null;
        const covered = tree !== null && tree.length === len;
        check(state, { full: true, tree: covered ? tree : null });
        if (!covered && ++this.tries < TREE_TRIES) this.schedule();
      }
    },
  );
}
