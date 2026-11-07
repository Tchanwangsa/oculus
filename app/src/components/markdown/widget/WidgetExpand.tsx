import { useEffect, useRef, useState } from "react";
import { X } from "@phosphor-icons/react";
import { Dialog, DialogCanvas, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { IconAction } from "@/components/markdown/embed/IconAction";
import { useWidgetFrame } from "./useWidgetFrame";
import { AskChoice, AskRunning } from "./ask/AskLine";
import type { AskControls } from "./ask/useWidgetAsk";

/**
 * The widget filling the window: a second frame at the inline frame's width
 * (so it lays out the same), scaled with a transform to fit the window —
 * never scrolled, never re-laid out wider. A transform, not CSS `zoom`, so
 * pointer input inside the frame stays exact. Its state goes through
 * `onState` like the inline frame's; `onClose` gets the last of it. A 3D
 * scene in it draws for as long as it is open. Its requests share the
 * inline widget's `ask`, whose Allow and Stop move into the bar below, since
 * the dialog covers the card.
 */
export function WidgetExpand({
  doc,
  title,
  width,
  height,
  onState,
  ask,
  onClose,
}: {
  doc: string;
  title: string;
  /** The inline frame's width in CSS px. */
  width: number;
  /** Its height until this frame reports. */
  height: number;
  onState: (state: Record<string, unknown>) => void;
  ask: AskControls;
  onClose: () => void;
}) {
  const frame = useRef<HTMLIFrameElement>(null);
  const area = useRef<HTMLDivElement>(null);
  const [room, setRoom] = useState<{ w: number; h: number } | null>(null);
  // Always live while open: the inline frame is paused meanwhile.
  const report = useWidgetFrame(frame, doc, onState, height, true, ask.handle);
  const asks = ask.holding || ask.capped || ask.asking;

  useEffect(() => {
    const el = area.current;
    if (!el) return;
    const measure = () => setRoom({ w: el.clientWidth, h: el.clientHeight });
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    measure();
    return () => observer.disconnect();
  }, []);

  const h = Math.max(1, report.height ?? height);
  const w = Math.max(1, width);
  const scale = room ? Math.min(room.w / w, room.h / h) : 0;

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogCanvas>
        <DialogTitle className="sr-only">{title}</DialogTitle>
        <DialogDescription className="sr-only">The widget, scaled to the window. Escape to close.</DialogDescription>
        <div ref={area} className="m-9 mb-20 flex min-h-0 flex-1">
          <div
            className="m-auto flex-none overflow-hidden rounded-xl border border-border bg-card"
            style={{ width: w * scale, height: h * scale, visibility: scale ? undefined : "hidden" }}
          >
            <iframe
              ref={frame}
              // Same sandbox as the inline frame: never `allow-same-origin`.
              sandbox="allow-scripts"
              srcDoc={doc}
              title={title}
              style={{ width: w, height: h, transform: `scale(${scale || 1})`, transformOrigin: "0 0" }}
              className="block border-0"
            />
          </div>
        </div>
        <div className="pointer-events-none absolute inset-x-0 bottom-6 flex justify-center px-6">
          <div className="pointer-events-auto flex min-w-0 items-center gap-0.5 rounded-full border border-border bg-card/90 p-1 pl-3 text-xs text-muted-foreground shadow-md backdrop-blur-sm">
            <span className="max-w-80 min-w-0 truncate">{title}</span>
            {asks && (
              <>
                <div className="mx-1 h-4 w-px shrink-0 bg-border" />
                <div className="flex min-w-0 items-center gap-1.5 text-foreground">
                  <AskChoice ask={ask} />
                  <AskRunning ask={ask} />
                </div>
              </>
            )}
            <div className="mx-1 h-4 w-px shrink-0 bg-border" />
            <IconAction label="Close" onClick={onClose} size="icon-sm">
              <X size={14} />
            </IconAction>
          </div>
        </div>
      </DialogCanvas>
    </Dialog>
  );
}
