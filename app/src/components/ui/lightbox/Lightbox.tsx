import { useEffect, useState } from "react";
import {
  Dialog,
  DialogCanvas,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Viewer } from "@/components/ui/lightbox/Viewer";
import type { LightboxSize } from "@/components/ui/lightbox/constants";

export type { LightboxSize };

/**
 * A full-window, zoomable, pannable viewer for something drawn small — a
 * mermaid figure (`DiagramLightbox`) or an attached picture (`ImageLightbox`).
 *
 * Panning is the container's own scroll (free momentum, scrollbars, keys);
 * only the scale is a `transform`, on a host of fixed natural size inside a
 * layout box sized `natural × zoom`. A transform, not CSS `zoom` (see
 * docs/ui.md): under a transform pointer coords and rects share one space, so
 * the anchor maths in `Viewer` holds, and the content never re-lays out.
 */

export function Lightbox({
  size,
  open,
  onOpenChange,
  title,
  children,
  scrollerClassName,
  selectableSelector,
}: {
  size: LightboxSize;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Screen-reader only. */
  title: string;
  /** Drawn at `size`, scaled by a transform. */
  children: React.ReactNode;
  /** E.g. the diagram restores its labels' I-beam and selection here. */
  scrollerClassName?: string;
  /** A press on a match selects text instead of starting a pan. */
  selectableSelector?: string;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogCanvas
        // Focus the canvas, not the toolbar's first button, so keys work at once.
        onOpenAutoFocus={(e) => {
          e.preventDefault();
          (e.currentTarget as HTMLElement).querySelector<HTMLElement>("[data-canvas]")?.focus();
        }}
      >
        <DialogTitle className="sr-only">{title}</DialogTitle>
        <DialogDescription className="sr-only">
          Scroll or drag to pan, ⌘-scroll or pinch to zoom, Escape to close.
        </DialogDescription>
        <Viewer
          size={size}
          onClose={() => onOpenChange(false)}
          scrollerClassName={scrollerClassName}
          selectableSelector={selectableSelector}
        >
          {children}
        </Viewer>
      </DialogCanvas>
    </Dialog>
  );
}

/** A picture, opened out. Measures its natural size first (from cache) and
 *  mounts the viewer only then — a fit against 0×0 would open at a clamp. */
export function ImageLightbox({
  src,
  alt,
  open,
  onOpenChange,
}: {
  src: string;
  alt?: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [size, setSize] = useState<LightboxSize | null>(null);

  useEffect(() => {
    if (!open || !src) {
      setSize(null);
      return;
    }
    let live = true;
    const probe = new Image();
    probe.onload = () => {
      if (live) setSize({ width: probe.naturalWidth, height: probe.naturalHeight });
    };
    probe.src = src;
    return () => {
      live = false;
    };
  }, [open, src]);

  if (!open || !size) return null;

  return (
    <Lightbox size={size} open={open} onOpenChange={onOpenChange} title={alt || "Picture"}>
      <img src={src} alt={alt ?? ""} draggable={false} className="h-full w-full" />
    </Lightbox>
  );
}
