import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { DEFAULT_JOB_MODELS, type JobChain, type JobId } from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";
import { ChainPicker } from "./ChainPicker";
import { ChainEntryRow } from "./ChainEntryRow";
import { moveEntry, withEntry, withoutEntry } from "./chain";

/** Edits apply as they are made, through the same save as the job's row, so
 *  there is nothing to confirm or cancel. */
export function JobModelsDialog({
  job,
  label,
  chain,
  providers,
  onChange,
  onView,
  onClose,
}: {
  job: JobId;
  label: string;
  chain: JobChain;
  providers: PickerProvider[];
  /** Takes an edit to apply to the current chain, not a finished one, so
   *  two edits in a row both land. */
  onChange: (edit: (chain: JobChain) => JobChain) => void;
  onView: (provider: Provider) => void;
  onClose: () => void;
}) {
  const addProvider = chain[chain.length - 1]?.provider ?? DEFAULT_JOB_MODELS[job][0].provider;

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{label}</DialogTitle>
          <DialogDescription>
            Runs on the first model. If a run fails for any reason, the next one takes over.
            With none, the job is off.
          </DialogDescription>
        </DialogHeader>

        <div>
          {chain.length > 0 && (
            // Keyed by place, not model: a row whose model changes keeps its
            // picker mounted, so the menu stays open for the level.
            <ol>
              {chain.map((entry, i) => (
                <ChainEntryRow
                  key={i}
                  position={i + 1}
                  entry={entry}
                  providers={providers}
                  isFirst={i === 0}
                  isLast={i === chain.length - 1}
                  onPick={(pick) => onChange((c) => withEntry(c, i, pick))}
                  onView={onView}
                  onMove={(by) => onChange((c) => moveEntry(c, i, by))}
                  onRemove={() => onChange((c) => withoutEntry(c, i))}
                />
              ))}
            </ol>
          )}
          <div className="mt-1 pl-6">
            <ChainPicker
              providers={providers}
              entry={null}
              fallbackProvider={addProvider}
              onPick={(pick) => onChange((c) => withEntry(c, c.length, pick))}
              onView={onView}
              emptyLabel="Add a model"
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" size="sm" onClick={onClose}>
            Done
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
