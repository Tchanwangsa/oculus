import { useEffect, useRef, useState, type DragEvent, type RefObject } from "react";
import { watchNativeFileDrop } from "@/lib/ui/nativeFileDrop";
import { draggedEntries, endEntryDrag, planDrop, type DropPlan } from "@/lib/drive/drop";

/** A folder a drop can land on, as its element says: `data-drop-rel` and
 *  `data-drop-writable="true"` on folder cards, rows and tree rows. */
export interface DropTarget {
  rel: string;
  writable: boolean;
}

/** Where the drag is now: the folder it would land in, and whether it may. */
export interface DropHover extends DropTarget {
  ok: boolean;
}

function targetOf(el: Element | null, fallback: DropTarget | null): DropTarget | null {
  const hit = el?.closest<HTMLElement>("[data-drop-rel]");
  if (hit) return { rel: hit.dataset.dropRel ?? "", writable: hit.dataset.dropWritable === "true" };
  return fallback;
}

/**
 * Drops onto a Drive surface (the browser, the folder tree): Finder files
 * through the webview's native events (`useFileDrop`'s source) and Drive's own
 * entry drags through HTML5 events, both landing on the folder under the
 * pointer or, in empty space, `fallback` (the folder on screen; none for the
 * tree). The hover is reported before the drop so a refused folder says so
 * while the files are still held. Spread `handlers` on the surface's root.
 */
export function useDriveDrop(
  ref: RefObject<HTMLElement | null>,
  {
    fallback,
    onFiles,
    onEntries,
    onRefused,
  }: {
    fallback: DropTarget | null;
    onFiles: (paths: string[], target: DropTarget) => void;
    onEntries: (plan: Extract<DropPlan, { kind: "go" }>, target: DropTarget) => void;
    onRefused: () => void;
  },
) {
  const [hover, setHover] = useState<DropHover | null>(null);
  const latest = useRef({ fallback, onFiles, onEntries, onRefused });
  latest.current = { fallback, onFiles, onEntries, onRefused };

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const pointTarget = (at: { x: number; y: number }) => {
      const rect = el.getBoundingClientRect();
      return targetOf(document.elementFromPoint(rect.left + at.x, rect.top + at.y), latest.current.fallback);
    };
    try {
      return watchNativeFileDrop(
        el,
        (paths, at) => {
          // An entry drag inside the window reaches here with no paths.
          const target = paths.length && at ? pointTarget(at) : null;
          setHover(target ? { ...target, ok: target.writable } : null);
        },
        (paths, at) => {
          if (!paths.length) return;
          const target = pointTarget(at);
          if (!target) return;
          if (target.writable) latest.current.onFiles(paths, target);
          else latest.current.onRefused();
        },
        8,
      );
    } catch {
      /* A plain browser has no native file events. */
    }
  }, [ref]);

  const handlers = {
    onDragOver(e: DragEvent) {
      const entries = draggedEntries(e.dataTransfer);
      if (!entries) return;
      const target = targetOf(e.target as Element, latest.current.fallback);
      if (!target) return setHover(null);
      const plan = planDrop(entries, target);
      setHover({ ...target, ok: plan.kind === "go" });
      if (plan.kind !== "go") return;
      e.preventDefault();
      e.dataTransfer.dropEffect = plan.copy.length && !plan.move.length ? "copy" : "move";
    },
    onDragLeave(e: DragEvent) {
      if (!ref.current?.contains(e.relatedTarget as Node | null)) setHover(null);
    },
    onDragEnd() {
      setHover(null);
      endEntryDrag();
    },
    onDrop(e: DragEvent) {
      const entries = draggedEntries(e.dataTransfer);
      setHover(null);
      if (!entries) return;
      e.preventDefault();
      const target = targetOf(e.target as Element, latest.current.fallback);
      endEntryDrag();
      if (!target) return;
      const plan = planDrop(entries, target);
      if (plan.kind === "go") latest.current.onEntries(plan, target);
      else if (plan.kind === "refuse") latest.current.onRefused();
    },
  };

  return { hover, handlers };
}
