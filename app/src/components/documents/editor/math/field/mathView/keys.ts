import type { FieldCommand } from "@/lib/maths";
import type { MathView } from "./index";

/** The model command for a key, or null to leave the key alone (the
 *  host's ⌘Z, the system's ⌘C, typing, which arrives as `beforeinput`). */
function commandFor(view: MathView, e: KeyboardEvent): FieldCommand | null {
  const mod = e.metaKey || e.ctrlKey;
  const extend = e.shiftKey;
  switch (e.key) {
    case "ArrowLeft":
      return mod ? { home: { extend } } : { left: { extend } };
    case "ArrowRight":
      return mod ? { end: { extend } } : { right: { extend } };
    case "ArrowUp":
    case "ArrowDown": {
      const xs = Array.from(view.measured.x);
      return e.key === "ArrowUp" ? { up: xs } : { down: xs };
    }
    case "Home":
      return { home: { extend } };
    case "End":
      return { end: { extend } };
    case "Backspace":
      return mod && !e.altKey ? "deleteLine" : "backspace";
    case "Delete":
      return "delete";
    case "Tab":
      return mod || e.altKey ? null : e.shiftKey ? "shiftTab" : "tab";
    case "Enter":
      return mod || e.altKey ? null : "enter";
    case "Escape":
      return "escape";
  }
  if (mod && !e.altKey && !e.shiftKey && e.code === "KeyA") return "selectAll";
  return null;
}

/** A key in the textarea: the host's first (`onKey`), then the model's.
 *  An IME's keys are its own. */
export function keyDown(view: MathView, e: KeyboardEvent) {
  if (view.dead || e.isComposing || view.composing || e.keyCode === 229) return;
  const stop = () => {
    e.preventDefault();
    e.stopPropagation();
  };
  if (view.host.onKey?.(e, view)) {
    stop();
    return;
  }
  const command = commandFor(view, e);
  if (!command) return;
  stop();
  view.run(command);
}
