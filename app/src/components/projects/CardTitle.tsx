import { useLayoutEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { CaretDown } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/**
 * A board card's task title (shared by `ProjectBoard` and `TasksBoard`),
 * folded to three lines with a Show more toggle only when the fold hid
 * something. The fold is measured against the rendered height, re-asked by a
 * `ResizeObserver` as the column width changes.
 */

const CLAMP_LINES = 3;

/** `leading-snug`, which both boards' titles carry; also the fallback when
 *  `line-height` computes to `normal`. */
const LINE_HEIGHT = 1.375;

/** A rounding guard in px, not a whole line: a title cut mid-token is wrong. */
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
      // `normal` would parse to NaN and make every card claim to overflow.
      const line = parseFloat(style.lineHeight) || parseFloat(style.fontSize) * LINE_HEIGHT;
      // `scrollHeight` ignores the clamp, so the toggle survives being used.
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
        // A native link drag would fight the card's pointer drag.
        draggable={false}
        className={cn(
          // `break-words` wraps a one-token title (a pasted URL) only because
          // the wrapper is `min-w-0`; `block` because `max-height` needs it.
          "block break-words leading-snug hover:underline",
          small ? "text-[11px]" : "text-xs",
          done ? "text-muted-foreground line-through" : "text-foreground",
          folded && "overflow-hidden",
        )}
        style={folded ? { maxHeight: `${CLAMP_LINES * LINE_HEIGHT}em` } : undefined}
      >
        {title}
      </Link>
      {long && (
        <button
          type="button"
          // `data-tab-skip` keeps ⌘-click off the card's tab; stopPropagation
          // keeps the card's click from navigating. Never cancel pointerdown
          // here — see docs/frontend.md (WebKit click).
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
