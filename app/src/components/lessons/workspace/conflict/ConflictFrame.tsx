import type { ReactNode } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import type { WorkspaceSaver } from "@/lib/ed/workspaceSaver";

import { ConflictBar } from "./ConflictBar";
import { ConflictDiff } from "./ConflictDiff";
import { TakeEdsDialog } from "./TakeEdsDialog";
import { useConflictResolve } from "./useConflictResolve";

/**
 * The open file's editor, and while it is in conflict the bar over it, the
 * comparison in its place on Compare, and the Take Ed's question. Editing
 * stays open during a conflict; only a resolve in flight makes it read-only,
 * so nothing typed is lost under the replaced file.
 */
export function ConflictFrame({
  challengeId,
  saver,
  path,
  serverPath,
  offline,
  onResolved,
  children,
}: {
  challengeId: number;
  saver: WorkspaceSaver;
  path: string;
  /** Ed's copy beside the file; null when it is in no conflict. */
  serverPath: string | null;
  offline: string | null;
  onResolved: (path: string) => void;
  /** The editor, read-only when `locked`. */
  children: (locked: boolean) => ReactNode;
}) {
  const c = useConflictResolve({ challengeId, saver, path, serverPath, onResolved });
  const inConflict = serverPath != null;
  const comparing = inConflict && c.texts != null;
  return (
    <div className="flex h-full min-h-0 flex-col">
      {inConflict && (
        <ConflictBar
          comparing={comparing}
          busy={c.busy}
          offline={offline}
          onCompare={c.compare}
          onKeepMine={c.keepMine}
          onTakeEds={c.askTakeEds}
        />
      )}
      {c.error && (
        <div className="shrink-0 px-3 pt-3">
          <Alert variant="destructive">
            <AlertTitle className="text-xs">{c.error.title}</AlertTitle>
            <AlertDescription className="text-xs">{c.error.message}</AlertDescription>
          </Alert>
        </div>
      )}
      <div className="min-h-0 flex-1">
        {comparing ? (
          c.texts === "loading" ? (
            <LoadingFill />
          ) : (
            c.texts && <ConflictDiff mine={c.texts.mine} eds={c.texts.eds} />
          )
        ) : (
          children(c.busy != null)
        )}
      </div>
      <TakeEdsDialog open={c.confirming} path={path} onConfirm={c.takeEds} onCancel={c.cancelTakeEds} />
    </div>
  );
}
