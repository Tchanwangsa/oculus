import { useLayoutEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { CaretDown } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/**
 * A task title on a board card: broken so it cannot escape the card, folded to
 * three lines so it cannot stretch one, and carrying a **Show more** toggle
 * only when folding it actually hid something.
 *
 * **One definition, two boards.** `ProjectBoard`'s `CardBody` and
 * `TasksBoard`'s `TaskCard` drew the same anchor with the same classes — the
 * project board a notch smaller for a subtask — and a title that folds on one
 * board and runs off the other reads as two different features. So the clamp,
 * the measurement and the toggle live here and both call it.
 *
 * Two shapes are the point of the file.
 *
 * `break-words` is what lets a title that is *one token* — a pasted
 * `https://cdn.…/saveCookie.html`, which has no break opportunity anywhere in
 * it — wrap instead of painting straight out through the card's right edge and
 * across the column beside it. It only works because this is a `min-w-0` flex
 * child: `overflow-wrap: break-word` leaves the element's *min-content* width
 * at the long word's full width, so a flex item still holding
 * `min-width: auto` would refuse to shrink to where the break has to happen.
 * (That is the pair `app/src/components/harness/Timeline.tsx` already runs a
 * thread of pasted paths through in this same WebKit; `overflow-wrap: anywhere`
 * would shrink the min-content too, and is not needed while the `min-w-0` is
 * there.)
 *
 * And the fold is **measured, not counted**: how many lines a title becomes is
 * the column's decision, not the string's, so the clamp is a `max-height` and
 * the question is whether the text is taller than it — re-asked by a
 * `ResizeObserver` so a column that changes width (the window, the page zoom)
 * changes its mind with it. `useOverflows` in `Timeline.tsx` is the same
 * pattern; the difference is that the threshold here is derived from the
 * element's own computed `line-height` rather than fixed in pixels, because the
 * two boards draw this at two sizes.
 *
 * **A dragged card is drawn twice** — once in its slot, once in the portalled
 * overlay riding the pointer — so the overlay's copy of this component has its
 * own fold state and starts folded even if the card in the list is expanded.
 * That is fine: the overlay is `aria-hidden`, it lives for the length of one
 * gesture, and its width comes from the captured rect, so the only visible
 * effect is that a card being dragged is as short as every other card.
 */

/** Lines of a title a folded card shows. Three is where a card stops reading
 *  as a card and starts reading as a paragraph with a due chip under it. */
const CLAMP_LINES = 3;

/** `leading-snug`, which both boards' titles carry. Used for the `em` clamp so
 *  one number serves the 12px title and the 11px subtask one, and as the
 *  fallback if `line-height` computes to `normal`. */
const LINE_HEIGHT = 1.375;

/**
 * How much hidden text is worth a toggle. Deliberately a rounding guard rather
 * than `Timeline`'s whole-line slack: a clipped question still says what it
 * was about, whereas a title cut mid-token is *wrong* — one hidden word of
 * `…/saveCookie.html` is the word that told the two cards apart.
 */
const SLACK = 2;

/** Whether the title is taller than the fold, asked of the element itself. */
function useClamped(title: string) {
  const ref = useRef<HTMLAnchorElement>(null);
  const [long, setLong] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      const style = getComputedStyle(el);
      // `line-height: 1.375` computes to px here, but `normal` would parse to
      // NaN and make every card claim to overflow.
      const line = parseFloat(style.lineHeight) || parseFloat(style.fontSize) * LINE_HEIGHT;
      // `scrollHeight` is the whole title even while `max-height` is clipping
      // it — and it does not depend on the clamp, so this answer is the same
      // folded or expanded and the toggle cannot disappear once it is used.
      setLong(el.scrollHeight > line * CLAMP_LINES + SLACK);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [title]);
  return [ref, long] as const;
}

export function CardTitle({
  title,
  href,
  done,
  small,
}: {
  title: string;
  /** The task's page. The anchor is the keyboard way in and the thing
   *  `newTabClicks.ts` reads for ⌘-click; the card around it navigates on a
   *  plain click of its own. */
  href: string;
  done: boolean;
  /** The project board's subtask size. */
  small?: boolean;
}) {
  const [ref, long] = useClamped(title);
  const [open, setOpen] = useState(false);
  const folded = long && !open;
  return (
    <div className="min-w-0 flex-1">
      <Link
        ref={ref}
        to={href}
        // An `<a href>` is draggable in WebKit in its own right, and a native
        // link drag starting under the card's pointer gesture would fight it.
        draggable={false}
        className={cn(
          // `block` so the clamp applies at all — `max-height` is ignored on an
          // inline box.
          "block break-words leading-snug hover:underline",
          small ? "text-[11px]" : "text-xs",
          done ? "text-muted-foreground line-through" : "text-foreground",
          folded && "overflow-hidden",
        )}
        // An exact multiple of the line box, so the cut lands between lines
        // rather than through the middle of one.
        style={folded ? { maxHeight: `${CLAMP_LINES * LINE_HEIGHT}em` } : undefined}
      >
        {title}
      </Link>
      {long && (
        <button
          type="button"
          // Three ways this control could be taken from under itself, and all
          // three are here. `data-tab-skip` stops `newTabClicks.ts` walking up
          // to the card's `data-tab-href` and opening a tab on ⌘-click; the
          // `stopPropagation` stops the card's own click navigating to the page
          // the toggle is expanding a title on. And nothing cancels
          // `pointerdown`: the card is a drag surface, and in WebKit a
          // cancelled press removes the `mousedown`/`mouseup` pair a `click` is
          // raised from — see `useCardDrag`'s `onPointerDown`. A press that
          // does not travel 4px is not a drag, so the gesture never sees this.
          data-tab-skip
          aria-expanded={open}
          onClick={(e) => {
            e.stopPropagation();
            setOpen((o) => !o);
          }}
          className="mt-0.5 flex cursor-pointer items-center gap-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
        >
          {open ? "Show less" : "Show more"}
          <CaretDown size={10} className={cn("transition-transform", open && "rotate-180")} />
        </button>
      )}
    </div>
  );
}
