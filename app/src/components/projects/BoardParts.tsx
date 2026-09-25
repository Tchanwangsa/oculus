import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { cn } from "@/lib/utils";
import { DRAG_SURFACE, type CardDragHandle, type CardDragState } from "@/hooks/useCardDrag";

/**
 * The chrome `ProjectBoard` and `TasksBoard` share: a column, a draggable card
 * that opens its task, and the lifted copy that rides the pointer. The boards
 * own what goes on a card and what a drop means; the gesture is `useCardDrag`.
 */

const BOARD_CARD = cn(
  "rounded-lg border border-border-subtle bg-card px-2.5 py-2 cursor-grab active:cursor-grabbing",
  "hover:border-border",
  // Selection is held off in CSS, not by cancelling the press — see CLAUDE.md
  // (WebKit: cancelling pointerdown kills the click).
  DRAG_SURFACE,
);

export function BoardColumn({
  drag,
  id,
  name,
  count,
  highlighted,
  footer,
  children,
}: {
  drag: CardDragHandle;
  id: string;
  name: string;
  /** Every card in the column, subtasks included — it must match what is drawn. */
  count: number;
  highlighted: boolean;
  footer?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section
      ref={drag.containerRef(id)}
      className={cn(
        "flex w-72 shrink-0 flex-col rounded-xl border bg-surface/40 transition-colors",
        highlighted ? "border-brand/50" : "border-border-subtle",
      )}
    >
      <div className="flex shrink-0 items-center gap-2 px-3 pb-1.5 pt-2.5">
        <span className="truncate text-[11px] font-medium text-muted-foreground">{name}</span>
        <span className="text-[11px] tabular-nums text-muted-foreground/60">{count}</span>
      </div>

      <div
        className={cn(
          "flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto px-2 pt-1",
          footer ? "pb-1" : "pb-2",
        )}
      >
        {count === 0 && (
          <p className="px-1 py-3 text-[11px] text-muted-foreground/60">Nothing here yet.</p>
        )}
        {children}
      </div>

      {footer && <div className="shrink-0 px-2 pb-2 pt-0.5">{footer}</div>}
    </section>
  );
}

/**
 * A card in a column. A plain `<article>` rather than an `<a>`/`<button>`:
 * it holds controls of its own, and WebKit repairs nested interactive content
 * by closing the outer control early (see `components/markdown/FileChip.tsx`).
 * `data-tab-href` gives ⌘-click a route from anywhere on it
 * (`app/src/lib/newTabClicks.ts`).
 */
export function BoardCard({
  drag,
  containerId,
  id,
  index,
  href,
  frozen = false,
  className,
  children,
}: {
  drag: CardDragHandle;
  containerId: string;
  id: number;
  index: number;
  href: string;
  /** Hold every card still, e.g. while the pointer is over a column where a
   *  drop would do nothing. */
  frozen?: boolean;
  className?: string;
  children: ReactNode;
}) {
  const navigate = useNavigate();
  const live = drag.drag;
  const grabbed = live?.id === id;
  const shift = grabbed || frozen ? 0 : drag.shiftFor(containerId, index);
  return (
    <article
      ref={drag.itemRef(containerId, id)}
      data-tab-href={href}
      onPointerDown={(e) => drag.onPointerDown(e, { id, containerId })}
      onClick={(e) => {
        // The title anchor and the Show more toggle keep their own click.
        if (e.target instanceof Element && e.target.closest("a[href], button")) return;
        navigate(href);
      }}
      // Only a click that ended a drag is swallowed; a plain click opens the task.
      onClickCapture={(e) => {
        if (!drag.didDrag()) return;
        e.preventDefault();
        e.stopPropagation();
      }}
      style={shift ? { transform: `translateY(${shift}px)` } : undefined}
      className={cn(
        BOARD_CARD,
        className,
        grabbed
          ? // Invisible but still holding the pointer capture, so its cursor is
            // the one on screen for the whole gesture.
            "cursor-grabbing opacity-0"
          : live
            ? "transition-transform duration-200 ease-out"
            : "transition-colors",
      )}
    >
      {children}
    </article>
  );
}

/**
 * The lifted card, drawn again in a fixed overlay because each column is its
 * own scroller and would clip a card translated towards the next one. Portalled
 * to `<body>`, so `z-50` competes with the palette and dialogs, not the board.
 */
export function LiftedCard({ live, children }: { live: CardDragState; children: ReactNode }) {
  return createPortal(
    <div
      aria-hidden
      className={cn(
        "pointer-events-none fixed z-50",
        // No easing while it tracks the pointer; after release it glides into
        // the slot the column is holding open.
        live.settling && "transition-transform duration-200 ease-out",
      )}
      style={{
        left: live.rect.left,
        top: live.rect.top,
        width: live.rect.width,
        transform: `translate(${live.dx}px, ${live.dy}px)`,
      }}
    >
      <article className={cn(BOARD_CARD, "shadow-md")}>{children}</article>
    </div>,
    document.body,
  );
}
