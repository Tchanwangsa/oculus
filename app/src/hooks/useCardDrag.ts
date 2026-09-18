import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

/**
 * A pointer-capture drag for vertical lists that span several containers — the
 * project board's columns today, the task table's rows next.
 *
 * It is `TopTabBar`'s gesture (`app/src/components/tabs/TopTabBar.tsx`)
 * generalised: press, and once the pointer has travelled past a small
 * threshold the item lifts and rides the pointer, its neighbours slide out of
 * its way, and the store is written once, on drop — after which the item
 * *settles*: it glides into the slot the list is about to put it in, and the
 * list is held in its old order until it gets there, so the write's re-read
 * lands invisibly ({@link CardDragState.settling}). That gesture is the one
 * proven to work in this WKWebView, and it is the look the board is held to.
 *
 * It lives in `app/src/hooks/` rather than in `app/src/lib/` or beside the
 * board because it is a hook — `lib/` holds no React in this app — and because
 * nothing in it knows what a task is: it deals in `(containerId, itemId)` and
 * hands back neighbour ids. The board and the table are two callers of one
 * gesture, not one component's private helper.
 *
 * **Why not HTML5 drag-and-drop**, which this replaces: `app/src/index.css`
 * sets `body { user-select: none }` and then hands `span`, `p`, `h1`–`h6`,
 * `li`, `td`, `th`, `input` and `textarea` `user-select: text` back. A card
 * carries a due chip and a subtask counter, both `<span>`, so a press that
 * landed on one started a *text selection* — and in WebKit a selection
 * pre-empts the element drag, so `dragstart` never fired and the card simply
 * would not lift. Which press did that was invisible to the user: the card
 * moved when they grabbed dead space and did nothing when they grabbed a chip.
 *
 * A drag library would be a dependency for tens of cards. This is the gesture.
 */

/** How far the pointer travels before a press becomes a drag. Big enough that
 *  a click on a card's title is never mistaken for one, small enough that the
 *  lift still feels like it happened when the hand moved. */
const THRESHOLD = 4;

/** How long a dropped item takes to glide from where the hand left it into the
 *  slot it was dropped in — the *settle*, which {@link CardDragState.settling}
 *  explains. Kept in step with the `duration-200` the callers put on the item
 *  itself, since one is the animation the other waits for. */
const SETTLE_MS = 200;

/** The longest the settle will hold the list still waiting for the new order
 *  to come back. A local SQLite write and its re-read land inside this with
 *  room to spare; the cap is there so a write that *fails* — or a caller that
 *  answered `true` and then wrote nothing — costs a spring-back rather than a
 *  table frozen in the shape it had before the drop. */
const SETTLE_CAP_MS = 600;

/**
 * Put this on whatever the gesture is dragged by — the whole card on the
 * board, the row block in the table.
 *
 * It is how the text selection described above is held off, and it has to be
 * CSS rather than a cancelled `pointerdown`: cancelling the press stops the
 * selection, but in WebKit it also stops the `click` that a card's title needs
 * (see {@link useCardDrag}'s `onPointerDown`). CSS stops the selection from
 * *beginning* and leaves the event sequence alone.
 *
 * The descendant half is the load-bearing half. `index.css` hands `span`, `p`,
 * `h1`–`h6`, `li`, `td`, `th` and the form fields `user-select: text` back
 * inside a `body { user-select: none }`, and a card is full of `<span>`s — the
 * due chip, the subtask counter.
 *
 * **The descendant half only started working once the resets were layered.**
 * That `select-text` rule reads as a base reset and used to be written
 * *outside* `index.css`'s `@layer base` block, and unlayered CSS outranks
 * every layer a utility can live in — so `[&_*]:select-none` sat in the DOM
 * and did nothing, and this constant had to carry `!important` on both halves
 * to be heard at all. The resets now live in the layer where the root
 * `CLAUDE.md` says they belong, so a plain utility wins. If a card ever stops
 * lifting because a press landed on one of its chips, check that first.
 */
export const DRAG_SURFACE = "select-none [&_*]:select-none";

/** The item a gesture starts on. Ids are per-container: a card knows which
 *  column it is currently drawn in, which is all the engine needs to find it. */
export interface CardDragItem {
  id: number;
  containerId: string;
}

/** The live gesture, for the caller to render. */
export interface CardDragState {
  id: number;
  /** Pointer travel since the press — what the lifted item is translated by. */
  dx: number;
  dy: number;
  /** The container the item was picked up from. */
  containerId: string;
  /** The container the pointer is over now, and so the one it would land in. */
  targetContainerId: string;
  /** Where in that container it would land, as an index into the container's
   *  items **with the dragged one taken out** — see {@link CardDrop}. */
  targetIndex: number;
  /** The item's viewport box as it was when the lift started. A caller whose
   *  list clips — the board's columns each scroll — needs this to draw the
   *  lifted copy in a fixed overlay instead of in place. */
  rect: { left: number; top: number; width: number; height: number };
  /**
   * The pointer is up, and this is the **settle**: the item is no longer
   * tracking the hand but gliding into the slot it was dropped in, with
   * `dx`/`dy` now that slot's offset rather than the pointer's travel.
   *
   * It exists because a drop is not the end of a move. The write goes to
   * SQLite and the new order comes back as a re-read, tens of milliseconds
   * later, so ending the gesture on the release played the move *backwards*:
   * every transform came off while the list was still in its old order, the
   * item snapped back into the slot it had been lifted out of, sat there, and
   * then jumped to its new slot when the re-read landed. Both halves of that
   * were visible — the revert and the jump — and on the table the row numbers
   * renumbered in the same frame as the jump, which is what made it read as a
   * flicker rather than as a move.
   *
   * Holding the picture across that gap, and gliding the item to the offset
   * its neighbours have already opened for it, makes the two coincide: when
   * the transforms finally come off, the list underneath is in the order the
   * screen is already showing, and the swap is invisible.
   *
   * Callers give the item a transform transition while this is true, and keep
   * drawing the list as it was *before* the drop — {@link useSettledList}.
   */
  settling: boolean;
}

/** What a completed gesture hands back. */
export interface CardDrop {
  id: number;
  /** Where it came from, so a caller can tell a reorder from a move. */
  from: string;
  /** Where it goes. */
  containerId: string;
  /** Its index among the destination's items, the dragged one excluded. */
  index: number;
  /**
   * The item ids either side of the landing slot, `null` at an end.
   *
   * The pair is the point of this callback: `moveTask`
   * (`app/src/lib/projects.ts`) takes two neighbours rather than an index,
   * because a `position` is the midpoint between them rather than a slot in a
   * renumbered column. The dragged item is excluded from its own neighbour
   * list first — the subtlety `siblingDropSlot` in
   * `app/src/components/projects/taskTree.ts` documents: dropping a card one
   * slot down means "after the card that is currently below me", and counting
   * itself as its own neighbour would take the midpoint of the gap it is
   * already sitting in and land it back where it started.
   *
   * These read the container's items in DOM order, which is all this hook can
   * see. A caller whose list is grouped — the board draws a subtask under its
   * parent — cannot use them as they stand and re-derives the pair from
   * {@link CardDrop.index} against the right subset instead.
   */
  before: number | null;
  after: number | null;
}

export interface CardDragHandle {
  /** `null` between gestures; set on every pointer move once lifted, and kept
   *  through the settle that follows the release — a caller that treats it as
   *  "the pointer is down" wants `!drag.settling` as well. */
  drag: CardDragState | null;
  /** Ref for a container — the box the pointer is hit-tested against. On the
   *  board that is the whole column, header and composer included, so a drop
   *  anywhere on a column counts as that column. */
  containerRef: (containerId: string) => (node: HTMLElement | null) => void;
  /** Ref for one draggable item. */
  itemRef: (containerId: string, itemId: number) => (node: HTMLElement | null) => void;
  onPointerDown: (e: ReactPointerEvent<HTMLElement>, item: CardDragItem) => void;
  /** How far a *non-grabbed* item at `index` in `containerId` should be
   *  translated to open (or close) the gap, in px. `index` is the item's
   *  position in the list the caller renders, which is the order the rects
   *  were captured in. 0 when it does not move, and 0 between gestures. */
  shiftFor: (containerId: string, index: number) => number;
  /** Whether the gesture that just ended crossed the threshold. A card's title
   *  is a `Link`, so a plain click must navigate and a click that was really a
   *  drag must not; this answers that without swallowing every click. Stays
   *  true until the next press. */
  didDrag: () => boolean;
}

/** One item's captured geometry. Nothing is re-measured during a drag, so
 *  these numbers are the whole world the gesture reasons about. */
interface ItemBox {
  id: number;
  left: number;
  top: number;
  width: number;
  height: number;
  mid: number;
}

interface Snapshot {
  containers: { id: string; rect: DOMRect }[];
  /** Per container, its items sorted top to bottom — which is the order they
   *  are rendered in, read back off the screen rather than tracked. */
  lists: Map<string, ItemBox[]>;
  /** The vertical gap between two items. One CSS value serves every column, so
   *  one measurement does too. */
  gap: number;
  from: { containerId: string; index: number; box: ItemBox };
}

/** The first adjacent gap found anywhere. A list of one has no gap to show, so
 *  a column holding only the grabbed card borrows the number from a column
 *  that does — they are the same `gap-*` utility. */
function measureGap(lists: Map<string, ItemBox[]>): number {
  for (const list of lists.values()) {
    for (let i = 1; i < list.length; i++) {
      const gap = list[i].top - (list[i - 1].top + list[i - 1].height);
      if (gap >= 0) return gap;
    }
  }
  return 0;
}

export interface CardDragOptions {
  /**
   * The list the caller draws, or anything whose identity changes when that
   * list has been re-read — `nodes` in the project's views, `tasks` in the
   * universal one.
   *
   * It is the settle's second hand. The glide is over when it has run its
   * {@link SETTLE_MS} *and* the new order has arrived, and this is how the
   * hook sees the second half of that without guessing at a write's round
   * trip. Leave it out and there is no settle: the gesture ends on the
   * release, the way it did before one existed.
   */
  settleOn?: unknown;
}

/**
 * Where the dropped item has to be by the time the re-read lands: the offset
 * from the box it was lifted out of to the slot it is about to occupy, in the
 * layout the list is about to have.
 *
 * It is read off the same captured rects the gap-opening uses, so the two
 * agree by construction — the item glides to exactly the offset its neighbours
 * have been holding open for it. `null` when there is nothing measured to land
 * against, which is a drop into an empty container: there the gesture just
 * ends, since a glide to an invented coordinate is worse than none.
 */
function settleTo(s: Snapshot, drop: CardDrop): { dx: number; dy: number } | null {
  const others = (s.lists.get(drop.containerId) ?? []).filter((b) => b.id !== drop.id);
  const sameList = drop.containerId === s.from.containerId;
  const step = s.from.box.height + s.gap;

  let top: number;
  if (drop.index >= others.length) {
    const last = others[others.length - 1];
    if (!last) return null;
    // Onto the end of the list. In its *own* list everything below the slot it
    // left has already slid up by a step, the last item included, so the
    // landing top comes up with it.
    top = last.top + last.height + s.gap - (sameList ? step : 0);
  } else if (sameList && drop.index > s.from.index) {
    // Downwards in its own list: the item that ends up above it is one it has
    // travelled past, so that item has slid up a step and the slot follows it.
    const above = others[drop.index - 1];
    top = above.top + above.height + s.gap - step;
  } else {
    // Everything else — upwards in its own list, or anywhere in another one —
    // lands on the captured top of the item that ends up below it, since that
    // is the item currently opening the gap.
    top = others[drop.index].top;
  }

  // Sideways only when the item changed lists, and measured from the
  // containers rather than from a neighbour: a card keeps whatever inset it
  // has (a subtask's is `ml-4`), so the column's own displacement is the only
  // honest number. A caller that registers no containers — the table, whose
  // lists nest — has no sideways travel to make anyway.
  const fromBox = s.containers.find((c) => c.id === s.from.containerId);
  const toBox = s.containers.find((c) => c.id === drop.containerId);
  return {
    dx: fromBox && toBox ? toBox.rect.left - fromBox.rect.left : 0,
    dy: top - s.from.box.top,
  };
}

/**
 * The list a caller draws while a drop settles: the one the gesture was
 * measured against, held until the hook lets go.
 *
 * The re-read lands *mid-glide* — that is the whole point of the settle — and
 * honouring it there would draw the new order underneath transforms that were
 * worked out for the old one, displacing everything twice. So the caller keeps
 * drawing the list it had when the pointer went up, and takes the new one in
 * the same commit the transforms come off in, where the two are the same
 * picture.
 *
 * Holding the previous value in a ref is the standard shape for that, and the
 * write is safe under a double render: the value written is the one already
 * being rendered.
 */
export function useSettledList<T>(list: T, drag: CardDragState | null): T {
  const held = useRef(list);
  if (!drag?.settling) held.current = list;
  return held.current;
}

/**
 * @param onDrop Called once, synchronously, when a gesture lands somewhere
 * other than where it started. **Answer `true` if it wrote something**: a
 * refusal (a re-parent the table cannot write, a same-kind move the universal
 * board has no order for) means no new order is coming, and the item springs
 * back instead of gliding into a slot nothing will fill.
 */
export function useCardDrag(
  onDrop: (drop: CardDrop) => boolean | void,
  options: CardDragOptions = {},
): CardDragHandle {
  const [drag, setDrag] = useState<CardDragState | null>(null);

  const containers = useRef(new Map<string, HTMLElement>());
  const items = useRef(new Map<string, { containerId: string; id: number; node: HTMLElement }>());
  // The ref callbacks are cached by key so their identity is stable across
  // renders. Without that React would tear down and re-register every card's
  // ref on every pointer move — this hook re-renders the whole list on each
  // one — for no gain.
  const containerCbs = useRef(new Map<string, (node: HTMLElement | null) => void>());
  const itemCbs = useRef(new Map<string, (node: HTMLElement | null) => void>());

  const snapshot = useRef<Snapshot | null>(null);
  const moved = useRef(false);
  /** Ends the gesture in flight, if there is one — see the unmount effect. */
  const teardown = useRef<(() => void) | null>(null);
  // The callback is read at drop time rather than closed over at press time,
  // so a caller that rebuilds it each render still gets the current one.
  const dropRef = useRef(onDrop);
  dropRef.current = onDrop;

  /** The settle in flight: the `settleOn` value as it was at the drop, and when
   *  the glide started. `null` whenever there is no settle. */
  const settle = useRef<{ key: unknown; at: number } | null>(null);
  const settleTimer = useRef<number | null>(null);
  // Read at drop time rather than closed over, for the same reason `onDrop` is.
  const settleKey = useRef(options.settleOn);
  settleKey.current = options.settleOn;

  /** End the gesture, settle and all, and put the list back under the caller's
   *  own control. Every path out goes through here. */
  const stop = useCallback(() => {
    if (settleTimer.current != null) {
      window.clearTimeout(settleTimer.current);
      settleTimer.current = null;
    }
    settle.current = null;
    snapshot.current = null;
    setDrag(null);
  }, []);

  // A view swapped out mid-drag (the Board/Table tabs sit right above this)
  // would otherwise leave the listeners and the capture behind it — and a
  // settle would leave its timer.
  useEffect(
    () => () => {
      teardown.current?.();
      if (settleTimer.current != null) window.clearTimeout(settleTimer.current);
    },
    [],
  );

  // The new order has arrived. End the settle — but not before the glide has
  // had its {@link SETTLE_MS}: taking the reordered list in mid-flight would
  // move the item twice, which is the jump the settle exists to remove. In
  // practice the re-read beats the glide and this waits out the remainder.
  useEffect(() => {
    const phase = settle.current;
    if (!phase || options.settleOn === phase.key) return;
    const left = Math.max(0, SETTLE_MS - (performance.now() - phase.at));
    const t = window.setTimeout(stop, left);
    return () => window.clearTimeout(t);
  }, [options.settleOn, stop]);

  const containerRef = (containerId: string) => {
    let cb = containerCbs.current.get(containerId);
    if (!cb) {
      cb = (node: HTMLElement | null) => {
        if (node) containers.current.set(containerId, node);
        else {
          containers.current.delete(containerId);
          containerCbs.current.delete(containerId);
        }
      };
      containerCbs.current.set(containerId, cb);
    }
    return cb;
  };

  const itemRef = (containerId: string, itemId: number) => {
    // `\0` as the separator, written as the escape rather than typed as the
    // byte it stands for: a container id cannot contain one, so the key is
    // unambiguous — but a raw NUL in the source makes the whole file read as
    // *binary* to grep, ripgrep and ugrep, which then skip it in silence. A
    // search for anything in this file came back empty rather than missing.
    const key = `${containerId}\0${itemId}`;
    let cb = itemCbs.current.get(key);
    if (!cb) {
      cb = (node: HTMLElement | null) => {
        if (node) items.current.set(key, { containerId, id: itemId, node });
        else {
          items.current.delete(key);
          itemCbs.current.delete(key);
        }
      };
      itemCbs.current.set(key, cb);
    }
    return cb;
  };

  /**
   * Every rect, once, at the moment of the lift.
   *
   * Nothing reflows during a drag — the grabbed item keeps its slot and the
   * others only move by `transform` — so measuring again mid-gesture could
   * only read the *displaced* positions back in and set the maths chasing
   * itself. The cost is that a list scrolled or resized mid-drag is worked out
   * against where it used to be, which is the trade the tab strip makes too.
   */
  const capture = (item: CardDragItem): Snapshot | null => {
    const boxes: { id: string; rect: DOMRect }[] = [];
    for (const [id, node] of containers.current) {
      boxes.push({ id, rect: node.getBoundingClientRect() });
    }
    const lists = new Map<string, ItemBox[]>();
    for (const entry of items.current.values()) {
      const r = entry.node.getBoundingClientRect();
      const box: ItemBox = {
        id: entry.id,
        left: r.left,
        top: r.top,
        width: r.width,
        height: r.height,
        mid: r.top + r.height / 2,
      };
      const list = lists.get(entry.containerId);
      if (list) list.push(box);
      else lists.set(entry.containerId, [box]);
    }
    for (const list of lists.values()) list.sort((a, b) => a.top - b.top);

    const own = lists.get(item.containerId);
    const index = own ? own.findIndex((b) => b.id === item.id) : -1;
    if (!own || index < 0) return null;
    return {
      containers: boxes,
      lists,
      gap: measureGap(lists),
      from: { containerId: item.containerId, index, box: own[index] },
    };
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLElement>, item: CardDragItem) => {
    if (e.button !== 0) return;

    // **Nothing is cancelled here, and that is deliberate.** This handler used
    // to open with `e.preventDefault()`, to stop the text selection described
    // at the top of this file from starting. It did — and it also stopped the
    // card's title from being clickable at all.
    //
    // The spec's line is that cancelling `pointerdown` suppresses the
    // compatibility mouse events but leaves `click` alone. WebKit does not
    // behave that way: a `click` is raised from a `mousedown`/`mouseup` pair
    // on the same element, and with the `mousedown` suppressed there is no
    // pair to raise one from. So no `click` reached the `<Link>` inside the
    // card and the only way into a task's page was gone — deterministically,
    // on every card, which is how it was noticed.
    //
    // The selection is held off by {@link DRAG_SURFACE} instead, which is CSS
    // and therefore stops a selection *beginning* rather than cancelling the
    // event that would have begun it. Callers put it on the drag surface.
    // A hand back on the list interrupts a glide: the press is the newer
    // intent, and the settle's frozen order must not outlive it.
    if (settle.current) stop();
    moved.current = false;
    const el = e.currentTarget;
    const pointerId = e.pointerId;
    const startX = e.clientX;
    const startY = e.clientY;
    let snap: Snapshot | null = null;
    let latest: CardDrop | null = null;
    // The pointer between containers — in a column's header, in the gutter —
    // is not a change of mind, so the last container it was genuinely over
    // stands until it is over another one.
    let lastTarget = item.containerId;

    const onMove = (ev: PointerEvent) => {
      const dx = ev.clientX - startX;
      const dy = ev.clientY - startY;
      if (!snap && Math.hypot(dx, dy) >= THRESHOLD) {
        snap = capture(item);
        snapshot.current = snap;
        moved.current = snap != null;
      }
      const s = snap;
      if (!s) return;

      // **The selection guard, and why it is here rather than in CSS.**
      //
      // {@link DRAG_SURFACE} makes the card itself unselectable, which stops a
      // selection *starting* on the due chip or the counter. It cannot stop one
      // ending somewhere else: WebKit will happily anchor a selection at the
      // nearest selectable position and then extend it across every `<span>`,
      // `<p>` and `<li>` the pointer travels over — and `index.css` hands all
      // of those `user-select: text` back inside `body { user-select: none }`.
      // So a drag that crossed two columns smeared a highlight over the whole
      // board behind the card.
      //
      // Cancelling the *move* is the targeted cure. In WebKit a selection is
      // extended by the compatibility `mousemove`, and cancelling `pointermove`
      // suppresses that while leaving `mousedown`/`mouseup` — and so the
      // `click` a card's title needs — untouched. That is exactly the
      // distinction the `onPointerDown` comment below was written about: the
      // press must not be cancelled, the move must. `removeAllRanges` then
      // clears anything that was already anchored before the lift.
      ev.preventDefault();
      const selection = document.getSelection();
      if (selection && !selection.isCollapsed) selection.removeAllRanges();

      for (const c of s.containers) {
        if (
          ev.clientX >= c.rect.left &&
          ev.clientX <= c.rect.right &&
          ev.clientY >= c.rect.top &&
          ev.clientY <= c.rect.bottom
        ) {
          lastTarget = c.id;
          break;
        }
      }

      // Where it would land: the grabbed item's own centre against the
      // captured midpoints of the destination's items — the item's centre and
      // not the pointer's, because the pointer may have grabbed it by a corner.
      const centre = s.from.box.mid + dy;
      const others = (s.lists.get(lastTarget) ?? []).filter((b) => b.id !== item.id);
      let index = 0;
      while (index < others.length && others[index].mid < centre) index++;

      latest = {
        id: item.id,
        from: s.from.containerId,
        containerId: lastTarget,
        index,
        before: index > 0 ? others[index - 1].id : null,
        after: index < others.length ? others[index].id : null,
      };
      setDrag({
        id: item.id,
        dx,
        dy,
        containerId: s.from.containerId,
        targetContainerId: lastTarget,
        targetIndex: index,
        rect: s.from.box,
        settling: false,
      });
    };

    const end = () => {
      el.removeEventListener("pointermove", onMove);
      el.removeEventListener("pointerup", end);
      el.removeEventListener("pointercancel", end);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      if (el.hasPointerCapture(pointerId)) el.releasePointerCapture(pointerId);
      teardown.current = null;

      const drop = latest;
      const s = snap;
      snap = null;
      // A cancel commits the same as a release: the pointer is being taken
      // away by the OS rather than given up by the hand, and the last slot the
      // card was shown in is the answer the user was looking at.
      if (!drop || !s) return stop();
      // Landing in the slot it started in is nothing to write.
      if (drop.containerId === s.from.containerId && drop.index === s.from.index) {
        return stop();
      }
      // The write first, and the glide only if there was one. A caller that
      // refused the drop has no new order coming, so there is nothing for the
      // item to glide into and it springs back instead — see this hook's
      // `@param onDrop`. A caller that named no `settleOn` never glides at all.
      const wrote = dropRef.current(drop) === true;
      const to = wrote && settleKey.current !== undefined ? settleTo(s, drop) : null;
      if (!to) return stop();
      settle.current = { key: settleKey.current, at: performance.now() };
      settleTimer.current = window.setTimeout(stop, SETTLE_CAP_MS);
      // Only the offset and the phase change: the item keeps the box it was
      // lifted from, so the caller's overlay stays where it is and the
      // neighbours keep the gap `shiftFor` has them holding open.
      setDrag((prev) =>
        prev ? { ...prev, dx: to.dx, dy: to.dy, settling: true } : prev,
      );
    };

    el.setPointerCapture(pointerId);
    el.addEventListener("pointermove", onMove);
    el.addEventListener("pointerup", end);
    el.addEventListener("pointercancel", end);
    // Capture retargets every move and release to `el`, so the element's own
    // listeners are enough — except when `el` is unmounted mid-gesture, which
    // releases the capture implicitly and leaves nothing to hear the release.
    // These are the net; whichever fires first removes both sets.
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    teardown.current = end;
  };

  const shiftFor = (containerId: string, index: number): number => {
    const s = snapshot.current;
    if (!drag || !s) return 0;
    // One card's worth of room: what the grabbed card takes out of the list it
    // leaves, and what has to open up in the one it is joining.
    const step = s.from.box.height + s.gap;
    const source = containerId === drag.containerId;
    const target = containerId === drag.targetContainerId;
    if (source && target) {
      // A reorder inside one list: only the run between the old slot and the
      // new one moves, and it moves towards the slot being vacated.
      if (s.from.index < index && index <= drag.targetIndex) return -step;
      if (drag.targetIndex <= index && index < s.from.index) return step;
      return 0;
    }
    // Across two lists: the one it left closes up below it, the one it is over
    // opens at the landing slot.
    if (source) return index > s.from.index ? -step : 0;
    if (target) return index >= drag.targetIndex ? step : 0;
    return 0;
  };

  return {
    drag,
    containerRef,
    itemRef,
    onPointerDown,
    shiftFor,
    didDrag: () => moved.current,
  };
}
