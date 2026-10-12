import { useState } from "react";
import { startFirstSync } from "./library/firstSync";
import { MineruCard } from "./library/MineruCard";
import { SubjectChecklist } from "./library/SubjectChecklist";
import { SyncProgressLine } from "./library/SyncProgressLine";
import { useCanvasSubjects } from "./library/useCanvasSubjects";
import { VoyageRow } from "./library/VoyageRow";
import { STEP_META } from "./stepMeta";
import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

/**
 * The library's two keys and the subjects to sync (docs/onboarding.md).
 * Both keys are optional, so Next is always enabled. Leaving the step, by
 * Next or Skip, starts the first sync when Canvas is connected and a subject
 * is selected — after the key, so PDFs don't fail for a missing token.
 */
export function LibraryStep({ onNext, onSkip }: StepProps) {
  const { canvas, subjects, fetching, error, toggle, selectedCount } = useCanvasSubjects();
  const [leaving, setLeaving] = useState(false);
  const willSync = canvas === true && selectedCount > 0;

  const leave = (advance: () => void) => async () => {
    if (leaving) return;
    setLeaving(true);
    if (willSync) await startFirstSync();
    advance();
  };

  return (
    <StepFrame
      title="Set up your library"
      description="Add the keys that turn your PDFs into text and let the agent search pages, then pick the subjects to sync."
      icon={STEP_META.library.icon}
      onNext={leave(onNext)}
      onSkip={leave(onSkip)}
      nextLabel={willSync ? "Start syncing" : "Next"}
      nextDisabled={leaving}
    >
      <div className="flex flex-col gap-3">
        <MineruCard />
        <VoyageRow />
      </div>
      {canvas === true && (
        <SubjectChecklist
          subjects={subjects}
          fetching={fetching}
          error={error}
          onToggle={toggle}
        />
      )}
      {canvas === false && (
        <p className="text-xs text-muted-foreground">
          Connect Canvas to pick subjects. Nothing syncs until you do: go back to connect it, or
          sync later from the Sync page.
        </p>
      )}
      <SyncProgressLine />
    </StepFrame>
  );
}
