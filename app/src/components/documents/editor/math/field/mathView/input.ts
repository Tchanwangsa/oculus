import type { MathView } from "./index";

/**
 * Typing reaches the view as `beforeinput` on its textarea, cancelled so
 * the textarea stays empty: `insertText` is one `Insert`. An IME composes
 * in the textarea (its text drawn at the caret by `preedit`) and commits
 * once, at `compositionend`. Undo and redo bubble on to the host; paste is
 * the host's `paste` listener.
 */
export function beforeInput(view: MathView, e: InputEvent) {
  switch (e.inputType) {
    case "historyUndo":
    case "historyRedo":
      return;
    case "insertCompositionText":
    case "deleteCompositionText":
    case "insertFromComposition":
      if (view.composing) return;
      break;
    case "insertText":
      if (e.isComposing || view.composing) return;
      e.preventDefault();
      // WebKit can follow `compositionend` with the same text as typing.
      if (view.dead || !e.data || e.data === view.composed) return;
      view.run({ insert: e.data });
      return;
  }
  e.preventDefault();
}

export function compositionStart(view: MathView) {
  view.composing = true;
  view.composed = null;
  view.preedit.textContent = "";
  view.preedit.hidden = false;
}

/** The IME's text so far, at the caret; outside a composition the
 *  textarea is emptied of anything that got in. */
export function inputEvent(view: MathView) {
  if (view.composing) view.preedit.textContent = view.input.value;
  else if (view.input.value) view.input.value = "";
}

export function compositionEnd(view: MathView, e: CompositionEvent) {
  view.composing = false;
  view.preedit.hidden = true;
  view.preedit.textContent = "";
  view.input.value = "";
  const text = e.data;
  if (!text || view.dead) return;
  view.composed = text;
  window.setTimeout(() => {
    if (view.composed === text) view.composed = null;
  }, 0);
  view.run({ insert: text });
}
