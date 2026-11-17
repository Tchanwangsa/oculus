import { DRAG_SURFACE } from "@/hooks/gestures/usePointerDrag";
import { cn } from "@/lib/utils";
import type { DropState } from "./itemProps";

/** Every card's frame. `DRAG_SURFACE` keeps a press on its text from
 *  selecting instead of lifting the card. */
export const CARD = cn(
  "relative cursor-default rounded-xl border bg-card outline-none transition-[background-color,border-color] duration-100",
  DRAG_SURFACE,
);

/** Picked, a folder lit for a drop it takes, or faded for one it refuses.
 *  Only colours transition: a fading card would promote a layer and step
 *  its glyph at fractional zoom (`docs/ui.md`). */
export function cardState(selected: boolean, drop: DropState): string {
  if (drop === "ok") return "border-brand bg-brand/10";
  if (drop === "refused") return "border-border-subtle opacity-50";
  return selected ? "border-brand/50 bg-brand/5" : "border-border-subtle hover:bg-surface";
}
