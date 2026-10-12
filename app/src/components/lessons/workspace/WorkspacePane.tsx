import { useEffect, useRef, useState } from "react";

import { FileViewer } from "@/components/files/FileViewer";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import type { DbFile } from "@/lib/db";
import { WORKSPACE_EVENT, type WorkspaceEvent, type WorkspaceStatus } from "@/lib/ed/workspace";
import { holdWorkspace, type WorkspaceHold } from "@/lib/ed/workspaceHandles";
import type { SaveState } from "@/lib/ed/workspaceSaver";
import { openFileSmart } from "@/lib/files/openFile";
import { cn } from "@/lib/utils";

import { WorkspaceEditor } from "./WorkspaceEditor";
import { WorkspaceStatusLine } from "./WorkspaceStatusLine";

/** The brief's column: Ed's Description, beside the editor. */
const BRIEF_PANEL = {
  defaultWidth: 420,
  minWidth: 280,
  maxWidth: 760,
  storageKey: "oculus-ed-workspace-brief",
} as const;

/**
 * A `code` slide as Ed shows it: the slide's brief (its `.md`) on the left,
 * the challenge's files in tabs over `CodeEditor` on the right, saving to Ed
 * as you type (`lib/ed/workspaceSaver.ts`). The session opens on mount and
 * closes after the last pane on it goes (`holdWorkspace`); `ed-workspace`
 * events for this challenge update the connection, the file list, and the
 * open file when another client commits.
 */
export function WorkspacePane({
  challengeId,
  brief,
  files,
}: {
  challengeId: number;
  /** The slide's `.md`. */
  brief: DbFile;
  /** The subject's files, for links in the brief. */
  files: DbFile[];
}) {
  const panel = useResizablePanel(BRIEF_PANEL);
  const [hold, setHold] = useState<WorkspaceHold | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  const [paths, setPaths] = useState<string[] | null>(null);
  const [active, setActive] = useState<string | null>(null);
  const [connection, setConnection] = useState<WorkspaceStatus>({ state: "connecting", reason: null });
  const [save, setSave] = useState<SaveState>({ state: "idle" });
  const [notice, setNotice] = useState<string | null>(null);
  const [remote, setRemote] = useState(0);
  /** Each file's undo history and caret, kept across tab switches. */
  const memories = useRef(new Map<string, unknown>());

  useEffect(() => {
    const h = holdWorkspace(challengeId);
    setHold(h);
    setOpenError(null);
    setSave(h.saver.status);
    const off = h.saver.subscribe({ status: setSave });
    let live = true;
    h.info
      .then((info) => {
        if (!live) return;
        setPaths(info.files);
        setConnection(info.status);
      })
      .catch((e) => live && setOpenError(String(e)));
    return () => {
      live = false;
      off();
      h.release();
    };
  }, [challengeId]);

  /** The open tab: the chosen file while it is listed, else the first. */
  const shown = active != null && paths?.includes(active) ? active : (paths?.[0] ?? null);

  useTauriEvent<WorkspaceEvent>(WORKSPACE_EVENT, ({ payload }) => {
    if (payload.challengeId !== challengeId) return;
    switch (payload.kind) {
      case "status":
        setConnection(payload.status);
        if (payload.status.state === "online") setNotice(null);
        break;
      case "files":
        setPaths(payload.files);
        hold?.saver.forgetClean((p) => payload.files.includes(p));
        break;
      case "commit":
        if (payload.ours) break;
        // Files not open read again when next opened.
        hold?.saver.forgetClean((p) => p === shown);
        setRemote((n) => n + 1);
        break;
      case "error":
        setNotice(payload.message);
        break;
    }
  });

  const select = (path: string) => {
    if (path === shown) return;
    void hold?.saver.flush();
    setActive(path);
  };

  return (
    <div className="flex min-h-0 flex-1">
      <section
        aria-label="Description"
        inert={panel.width === 0}
        style={{ width: panel.width }}
        className={cn(
          "flex min-h-0 shrink-0 flex-col overflow-hidden",
          !panel.dragging && "transition-[width] duration-200 ease-out",
        )}
      >
        <div style={{ width: panel.restWidth }} className="flex min-h-0 flex-1 flex-col">
          <FileViewer file={brief} files={files} onOpenFile={openFileSmart} />
        </div>
      </section>
      <ResizeHandle
        onMouseDown={panel.onMouseDown}
        dragging={panel.dragging}
        label="Resize description"
        className="-mx-0.5"
      />
      <section aria-label="Workspace" className="flex min-w-0 flex-1 flex-col border-l border-border-subtle">
        <div className="flex h-10 shrink-0 items-center gap-3 border-b border-border-subtle px-3">
          <div className="min-w-0 flex-1">
            {shown != null && paths && (
              <PillTabs scroll tabs={paths.map((p) => ({ value: p, label: p }))} value={shown} onChange={select} />
            )}
          </div>
          {!openError && <WorkspaceStatusLine connection={connection} save={save} />}
        </div>
        {(openError || save.state === "error" || notice) && (
          <div className="shrink-0 space-y-2 px-3 pt-3">
            {openError && (
              <Alert variant="destructive">
                <AlertTitle className="text-xs">Couldn't open the workspace</AlertTitle>
                <AlertDescription className="text-xs">{openError}</AlertDescription>
              </Alert>
            )}
            {save.state === "error" && (
              <Alert variant="destructive">
                <AlertTitle className="text-xs">Not saved to Ed</AlertTitle>
                <AlertDescription className="text-xs">{save.message}</AlertDescription>
              </Alert>
            )}
            {notice && (
              <Alert variant="warning">
                <AlertDescription className="text-xs">{notice}</AlertDescription>
              </Alert>
            )}
          </div>
        )}
        <div className="min-h-0 flex-1">
          {hold && shown != null ? (
            <WorkspaceEditor
              key={shown}
              challengeId={challengeId}
              saver={hold.saver}
              path={shown}
              remote={remote}
              memory={{
                read: () => memories.current.get(shown),
                write: (json) => memories.current.set(shown, json),
              }}
            />
          ) : paths === null ? (
            !openError && <LoadingFill />
          ) : (
            paths.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">This workspace has no files.</p>
          )}
        </div>
      </section>
    </div>
  );
}
