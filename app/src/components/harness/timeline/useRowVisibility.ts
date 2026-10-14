import { useLayoutEffect, type RefObject } from "react";
import { ANCHOR_ROW } from "@/hooks/ui/useStickToBottom";
import { onExpand } from "@/lib/ui/expandable";
import { anchorDrift, HELD_ANCHOR, pickAnchor, scrollParent, SKIPPED_CONTENT, zoomScale } from "@/lib/ui/scrollAnchor";

/** Move off-screen portal hosts out of the document while retaining their React
 *  state. Fixed-height row boxes remain in the scroll flow; revealing restores
 *  each host, its native scroll offsets, and its current content. */
export function useRowVisibility(rootRef: RefObject<HTMLDivElement | null>, threadId: number | null) {
  useLayoutEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const scroller = scrollParent(root);
    const heights = new Map<HTMLElement, number>();
    const rows = new Set<HTMLElement>();
    const requested = new Set<HTMLElement>();
    const contents = new Map<HTMLElement, {
      host: HTMLElement;
      scrolls: Map<HTMLElement, { top: number; left: number }>;
      onScroll: (e: Event) => void;
    }>();
    let expanded = false;

    const show = (row: HTMLElement) => {
      row.removeAttribute(SKIPPED_CONTENT);
      row.style.contentVisibility = "visible";
      row.style.containIntrinsicBlockSize = "";
      row.style.height = "";
      row.style.contain = "";
      const content = contents.get(row);
      if (!content || content.host.parentElement === row) return;
      row.appendChild(content.host);
      for (const [el, at] of content.scrolls) {
        el.scrollTop = at.top;
        el.scrollLeft = at.left;
      }
    };
    const selected = (row: HTMLElement) => {
      if (row.hasAttribute(HELD_ANCHOR) || row.contains(document.activeElement)) return true;
      const selection = window.getSelection();
      return !!selection?.rangeCount && !selection.isCollapsed && selection.getRangeAt(0).intersectsNode(row);
    };
    const resize = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const row = entry.target as HTMLElement;
        if (!row.hasAttribute(SKIPPED_CONTENT)) heights.set(row, entry.contentRect.height);
      }
    });
    const visibility = new IntersectionObserver((entries) => {
      // Observer rects include page zoom; intrinsic sizes are layout pixels.
      const scale = scroller ? zoomScale(scroller.getBoundingClientRect().height, scroller.offsetHeight) : 1;
      for (const entry of entries) {
        const row = entry.target as HTMLElement;
        if (entry.isIntersecting) requested.delete(row);
        if (expanded || entry.isIntersecting || requested.has(row) || selected(row)) {
          show(row);
        } else {
          // skip-offscreen supplies 4px of padding at each block edge.
          const height = heights.get(row) ?? Math.max(0, entry.boundingClientRect.height / scale - 8);
          row.style.containIntrinsicBlockSize = `${height}px`;
          row.style.height = `${height + 8}px`;
          row.style.contain = "size layout style paint";
          row.setAttribute(SKIPPED_CONTENT, "");
          row.style.contentVisibility = "hidden";
          const host = contents.get(row)?.host;
          // Detaching a frame destroys its browsing context. Media and frames
          // retain their connection; ordinary message DOM leaves the document.
          if (host && !host.querySelector("iframe, video, audio")) host.remove();
        }
      }
    }, { root: scroller, rootMargin: "200px 0px" });

    // Direct children only: appending history must not walk the maths DOM.
    const sync = () => {
      for (const row of rows) {
        if (row.parentElement === root) continue;
        visibility.unobserve(row);
        resize.unobserve(row);
        rows.delete(row);
        heights.delete(row);
        requested.delete(row);
        const content = contents.get(row);
        content?.host.removeEventListener("scroll", content.onScroll, true);
        contents.delete(row);
      }
      for (const child of root.children) {
        if (!(child instanceof HTMLElement) || !child.classList.contains("skip-offscreen") || rows.has(child)) continue;
        const host = child.firstElementChild;
        if (!(host instanceof HTMLElement)) continue;
        const scrolls = new Map<HTMLElement, { top: number; left: number }>();
        const onScroll = (e: Event) => {
          if (e.target instanceof HTMLElement && e.target.isConnected) {
            scrolls.set(e.target, { top: e.target.scrollTop, left: e.target.scrollLeft });
          }
        };
        contents.set(child, { host, scrolls, onScroll });
        host.addEventListener("scroll", onScroll, true);
        rows.add(child);
        if (expanded) show(child);
        resize.observe(child);
        visibility.observe(child);
      }
    };
    sync();
    const changes = new MutationObserver(sync);
    changes.observe(root, { childList: true });
    const off = onExpand(root, ({ upToId }) => {
      // Reveal a question before the rail reads its nested bubble's position.
      if (upToId != null) {
        for (const node of root.getElementsByClassName("thread-map-question-row")) {
          const row = node as HTMLElement;
          if (row.dataset.questionId !== String(upToId)) continue;
          requested.add(row);
          show(row);
          break;
        }
        return;
      }
      // Find and Select All need every row before their synchronous DOM walk.
      const anchor = scroller && pickAnchor(scroller, root.getElementsByClassName(ANCHOR_ROW));
      expanded = true;
      for (const row of rows) show(row);
      if (scroller && anchor) {
        const drift = anchorDrift(scroller, anchor, root);
        if (drift != null) scroller.scrollTop += drift;
      }
    });
    return () => {
      off();
      changes.disconnect();
      visibility.disconnect();
      resize.disconnect();
      for (const row of rows) {
        show(row);
        const content = contents.get(row);
        content?.host.removeEventListener("scroll", content.onScroll, true);
        row.removeAttribute(SKIPPED_CONTENT);
        row.style.contentVisibility = "";
        row.style.containIntrinsicBlockSize = "";
        row.style.height = "";
        row.style.contain = "";
      }
    };
  }, [rootRef, threadId]);
}
