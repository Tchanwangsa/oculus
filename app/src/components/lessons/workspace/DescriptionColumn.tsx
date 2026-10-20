import { useEffect, useState } from "react";

import { PillTabs } from "@/components/ui/table/PillTabs";
import type { DbFile } from "@/lib/db";
import type { Submission } from "@/lib/ed/workspace";
import { cn } from "@/lib/utils";

import { BriefMarkdown } from "./BriefMarkdown";
import { FeedbackPanel } from "./feedback/FeedbackPanel";

export type SideTab = "description" | "feedback";

export const SIDE_TABS: ReadonlyArray<{ value: SideTab; label: string }> = [
  { value: "description", label: "Description" },
  { value: "feedback", label: "Feedback" },
];

/**
 * Ed's left column: the slide's brief (`BriefMarkdown`), and beside it a Feedback tab when the
 * challenge marks or gives feedback. Feedback mounts on its first open and
 * then stays, so switching back and forth keeps what it loaded.
 */
export function DescriptionColumn({
  challengeId,
  slideId,
  brief,
  files,
  feedback,
  tab,
  onTab,
  marked,
  openWorkspaceFile,
  stacked,
}: {
  challengeId: number;
  slideId: number | null;
  brief: DbFile;
  files: DbFile[];
  /** Whether the Feedback tab exists. */
  feedback: boolean;
  tab: SideTab;
  onTab: (tab: SideTab) => void;
  marked: Submission | null;
  /** Opens a workspace file's tab from the brief's Files list. */
  openWorkspaceFile: (path: string) => boolean;
  /** The pane's own strip carries Description and Feedback, so this one is left out. */
  stacked: boolean;
}) {
  const shown: SideTab = feedback ? tab : "description";
  const [opened, setOpened] = useState(shown === "feedback");
  useEffect(() => {
    if (shown === "feedback") setOpened(true);
  }, [shown]);

  const viewer = <BriefMarkdown brief={brief} files={files} openWorkspaceFile={openWorkspaceFile} />;
  if (!feedback) return viewer;
  return (
    <>
      {!stacked && (
        <div className="flex h-10 shrink-0 items-center border-b border-border-subtle px-3">
          <PillTabs tabs={SIDE_TABS} value={shown} onChange={onTab} />
        </div>
      )}
      <div className={cn("flex min-h-0 flex-1 flex-col", shown !== "description" && "hidden")}>{viewer}</div>
      {opened && (
        <div className={cn("min-h-0 flex-1", shown !== "feedback" && "hidden")}>
          <FeedbackPanel challengeId={challengeId} slideId={slideId} active={shown === "feedback"} marked={marked} />
        </div>
      )}
    </>
  );
}
