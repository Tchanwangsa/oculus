import { useMemo } from "react";

import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { dataUrlText } from "@/lib/ed/runFiles";
import type { RunFile } from "@/lib/ed/workspace";

/** A text file the run left, read in a dialog as the program wrote it. */
export function RunFileText({ file, onClose }: { file: RunFile | null; onClose: () => void }) {
  const text = useMemo(() => (file?.dataUrl ? dataUrlText(file.dataUrl) : null), [file]);
  return (
    <Dialog open={file != null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{file?.name}</DialogTitle>
          <DialogDescription className="sr-only">A file the program made</DialogDescription>
        </DialogHeader>
        <pre
          data-selectable
          className="max-h-[60vh] overflow-auto rounded-md bg-surface px-3 py-2 font-mono text-xs whitespace-pre-wrap"
        >
          {text ?? ""}
        </pre>
      </DialogContent>
    </Dialog>
  );
}
