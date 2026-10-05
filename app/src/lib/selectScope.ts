/**
 * A drag-select stays in the scope it starts in. Text selects only inside a
 * select scope (`index.css`): a pane's page (`TabPane`), a dialog, a popover.
 * Once the pointer moves with the button held, every other scope stops
 * selecting until release, so a drag across the split divider never takes
 * the other pane's text. Engaged on move, not press, so a click restyles
 * nothing.
 */

const SCOPE =
  '[data-select-scope], [role="dialog"], [role="alertdialog"], [data-slot="popover-content"]';

export function containSelection(): () => void {
  const root = document.documentElement;
  // Undefined: no press. Null: a press outside any scope, which disables all.
  let pressed: Element | null | undefined;
  let engaged = false;

  const down = (e: PointerEvent) => {
    if (e.button !== 0) return;
    pressed = e.target instanceof Element ? e.target.closest(SCOPE) : null;
  };
  const move = (e: PointerEvent) => {
    if (engaged || pressed === undefined || !(e.buttons & 1)) return;
    engaged = true;
    pressed?.setAttribute("data-select-active", "");
    root.setAttribute("data-select-drag", "");
  };
  const up = () => {
    if (engaged) {
      pressed?.removeAttribute("data-select-active");
      root.removeAttribute("data-select-drag");
    }
    pressed = undefined;
    engaged = false;
  };

  window.addEventListener("pointerdown", down, true);
  window.addEventListener("pointermove", move, true);
  window.addEventListener("pointerup", up, true);
  window.addEventListener("pointercancel", up, true);
  window.addEventListener("blur", up);
  return () => {
    up();
    window.removeEventListener("pointerdown", down, true);
    window.removeEventListener("pointermove", move, true);
    window.removeEventListener("pointerup", up, true);
    window.removeEventListener("pointercancel", up, true);
    window.removeEventListener("blur", up);
  };
}
