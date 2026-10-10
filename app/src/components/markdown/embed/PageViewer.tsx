import { ArrowSquareOut, X } from "@phosphor-icons/react";
import { Dialog, DialogCanvas, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { IconAction } from "@/components/markdown/embed/IconAction";
import { openExternally } from "@/components/markdown/embed/shared";

/** The same page full-window, at full height — `DialogCanvas` with a
 *  toolbar in `ui/lightbox/Lightbox.tsx`'s grammar. No zoom: the page lays out to
 *  the window instead. */
export function PageViewer({
  doc,
  title,
  path,
  open,
  onOpenChange,
}: {
  doc: string;
  title: string;
  path: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogCanvas>
        <DialogTitle className="sr-only">{title}</DialogTitle>
        <DialogDescription className="sr-only">The page, full-window. Escape to close.</DialogDescription>
        <div className="flex min-h-0 flex-1 p-9 pb-20">
          <iframe
            // Same sandbox as the inline frame, and for the same reason.
            sandbox="allow-scripts"
            srcDoc={doc}
            title={title}
            className="min-h-0 w-full flex-1 rounded-lg border border-border bg-card"
          />
        </div>
        <div className="pointer-events-none absolute inset-x-0 bottom-6 flex justify-center px-6">
          <div className="pointer-events-auto flex min-w-0 items-center gap-0.5 rounded-full border border-border bg-card/90 p-1 pl-3 text-xs text-muted-foreground shadow-md backdrop-blur-sm">
            <span className="max-w-80 min-w-0 truncate" title={path}>
              {title}
            </span>
            <div className="mx-1 h-4 w-px shrink-0 bg-border" />
            <IconAction label="Open externally" onClick={() => openExternally(path)} size="icon-sm">
              <ArrowSquareOut size={14} />
            </IconAction>
            <IconAction label="Close" onClick={() => onOpenChange(false)} size="icon-sm">
              <X size={14} />
            </IconAction>
          </div>
        </div>
      </DialogCanvas>
    </Dialog>
  );
}
