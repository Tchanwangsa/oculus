import { useState } from "react";
import { Plus } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { DEFAULT_JOB_MODELS, type JobChain, type JobId, type JobSelection } from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";
import { ChainPicker } from "./ChainPicker";
import { ChainList } from "./ChainList";
import { withEntry, withoutEntry } from "./chain";

/** Edits apply as they are made, through the same save as the job's row, so
 *  there is nothing to confirm or cancel. A `perAgent` job lists a model per
 *  agent rather than fallbacks, and offers only those agents. */
export function JobModelsDialog({
  job,
  label,
  perAgent,
  chain,
  providers,
  onChange,
  onView,
  onClose,
}: {
  job: JobId;
  label: string;
  perAgent?: readonly Provider[];
  chain: JobChain;
  providers: PickerProvider[];
  /** Takes an edit to apply to the current chain, not a finished one, so
   *  two edits in a row both land. */
  onChange: (edit: (chain: JobChain) => JobChain) => void;
  onView: (provider: Provider) => void;
  onClose: () => void;
}) {
  const offered = perAgent ? providers.filter((p) => perAgent.includes(p.id)) : providers;
  const addProvider = chain[chain.length - 1]?.provider ?? DEFAULT_JOB_MODELS[job][0].provider;
  // The entry the open "add" menu just appended, so the menu stays on it for
  // its level and a second pick replaces it rather than appending again.
  // Cleared when the menu closes.
  const [added, setAdded] = useState<number | null>(null);
  const addPick = (pick: JobSelection) => {
    const at = added ?? chain.length;
    // A pick already in the chain moves there (`withEntry`), so find it again.
    const next = withEntry(chain, at, pick);
    setAdded(next.findIndex((e) => e.provider === pick.provider && e.model === pick.model));
    onChange((c) => withEntry(c, at, pick));
  };

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{label}</DialogTitle>
          <DialogDescription>
            {perAgent
              ? "A chat agent's helpers run on the first model here for that agent. An agent with none does the work itself."
              : "Runs on the first model. If a run fails for any reason, the next one takes over. With none, the job is off."}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-1">
          {chain.length > 0 && (
            <ChainList
              chain={chain}
              providers={offered}
              onChange={onChange}
              onView={onView}
              onPick={(i, pick) => onChange((c) => withEntry(c, i, pick))}
              onRemove={(i) => onChange((c) => withoutEntry(c, i))}
            />
          )}
          <ChainPicker
            providers={offered}
            entry={added === null ? null : (chain[added] ?? null)}
            fallbackProvider={addProvider}
            onPick={addPick}
            onView={onView}
            onOpenChange={(open) => !open && setAdded(null)}
            emptyLabel={
              chain.length > (added === null ? 0 : 1)
                ? perAgent
                  ? "Add a model for another agent"
                  : "Add a fallback"
                : "Add a model"
            }
            emptyIcon={<Plus className="size-3.5 shrink-0" />}
            keepEmptyTrigger
            className="ml-1"
          />
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
