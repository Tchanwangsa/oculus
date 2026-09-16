import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

export interface ReindexPrompt {
  /** The engine being moved to, named the way the control names it. */
  to: string;
  /** What wrote the vectors that are about to go — the stored model id. */
  from: string | null;
  vectors: number;
  files: number;
}

/**
 * Asked before the embedding model is changed, because the change is not
 * reversible by changing it back.
 *
 * The house rule this follows: a confirmation that states the consequence in
 * plain language beats a warning icon. So there is no icon and no red — the
 * numbers do the work, and they are the real ones, read from the index a
 * moment ago rather than described in the abstract.
 *
 * It is only raised when there is something to lose. An empty index makes this
 * an ordinary setting, and a dialog over nothing teaches people to click
 * through the one that matters.
 */
export function ReindexConfirmDialog({
  prompt,
  busy,
  onConfirm,
  onCancel,
}: {
  prompt: ReindexPrompt | null;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const vectors = prompt ? prompt.vectors.toLocaleString() : "";
  const files = prompt ? prompt.files.toLocaleString() : "";

  return (
    <Dialog open={!!prompt} onOpenChange={(open) => !open && !busy && onCancel()}>
      <DialogContent className="sm:max-w-md" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Re-index the whole library?</DialogTitle>
          <DialogDescription asChild>
            <div className="space-y-2.5">
              <p>
                Search runs against one embedding model at a time. The{" "}
                <span className="tabular-nums">{vectors}</span> page vectors you have now were made
                by {prompt?.from ?? "the previous model"}, and {prompt?.to} writes into a different
                space. The two cannot be compared — mixed together they would still produce a
                ranking, just a meaningless one.
              </p>
              <p>
                So switching deletes all <span className="tabular-nums">{vectors}</span> of them,
                and every one of the <span className="tabular-nums">{files}</span> indexed files
                has to be embedded again. Search finds nothing until that re-index has run, and
                only part of your library while it is running.
              </p>
              <p>
                Nothing else is touched: the PDFs and the text parsed out of them stay exactly as
                they are.
              </p>
            </div>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button onClick={onConfirm} disabled={busy}>
            {busy ? "Switching…" : "Switch and re-index"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
