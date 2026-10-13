import { useEffect, useMemo, useRef, useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import type { DbFile } from "@/lib/db";
import { splitListing } from "@/lib/ed/conflicts";
import {
  WORKSPACE_EVENT,
  type Submission,
  type WorkspaceEvent,
  type WorkspaceInfo,
  type WorkspaceStatus,
} from "@/lib/ed/workspace";
import { holdWorkspace, type WorkspaceHold } from "@/lib/ed/workspaceHandles";
import type { SaveState } from "@/lib/ed/workspaceSaver";
import { edUnavailableReason } from "@/lib/ed/workspaceStatus";
import { cn } from "@/lib/utils";

import { ConflictFrame } from "./conflict/ConflictFrame";
import { DescriptionColumn, type SideTab } from "./DescriptionColumn";
import { MarkButton } from "./mark/MarkButton";
import { MarkDialog } from "./mark/MarkDialog";
import { useMarkFlow } from "./mark/useMarkFlow";
import { RunButton } from "./terminal/RunButton";
import { TerminalDrawer, type DrawerTab } from "./terminal/TerminalDrawer";
import { useRunSession } from "./terminal/useRunSession";
import { useTerminalSession } from "./terminal/useTerminalSession";
import { WorkspaceEditor } from "./WorkspaceEditor";
import { WorkspaceStatusLine } from "./WorkspaceStatusLine";

/** The brief's column: Ed's Description, beside the editor. */
const BRIEF_PANEL = {
  defaultWidth: 420,
  minWidth: 280,
  maxWidth: 760,
  storageKey: "oculus-ed-workspace-brief",
} as const;

/** Ed's bottom drawer (terminal and Run), folded until first opened. */
const DRAWER_PANEL = {
  side: "bottom",
  defaultWidth: 240,
  minWidth: 120,
  maxWidth: 640,
  defaultCollapsed: true,
  storageKey: "oculus-ed-workspace-drawer",
} as const;

/**
 * A `code` slide as Ed shows it: the slide's brief (its `.md`) on the left
 * with its Feedback tab, the challenge's files in tabs over `CodeEditor` on
 * the right, saving to Ed as you type (`lib/ed/workspaceSaver.ts`), with
 * Run and Mark in its header and the terminal drawer under it, each as the
 * challenge's features allow. The session opens on mount and closes after
 * the last pane on it goes (`holdWorkspace`); `ed-workspace` events for this
 * challenge update the connection, the file list, the open file when
 * another client commits, and the files in conflict with Ed
 * (`lib/ed/conflicts.ts`), whose server copies never show as tabs.
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
  const drawer = useResizablePanel(DRAWER_PANEL);
  const [hold, setHold] = useState<WorkspaceHold | null>(null);
  const [info, setInfo] = useState<WorkspaceInfo | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  /** Rust's file list, server copies of conflicts included. */
  const [paths, setPaths] = useState<string[] | null>(null);
  /** Conflicts named by `conflict` events, until their copy leaves the list. */
  const [named, setNamed] = useState<ReadonlyMap<string, string>>(new Map());
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
        setInfo(info);
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

  const listing = useMemo(() => (paths ? splitListing(paths, named) : null), [paths, named]);
  const tabs = listing?.files ?? null;
  /** The open tab: the chosen file while it is listed, else the first. */
  const shown = active != null && tabs?.includes(active) ? active : (tabs?.[0] ?? null);

  useTauriEvent<WorkspaceEvent>(WORKSPACE_EVENT, ({ payload }) => {
    if (payload.challengeId !== challengeId) return;
    switch (payload.kind) {
      case "status":
        setConnection(payload.status);
        if (payload.status.state === "online") {
          setNotice(null);
          // Reconcile has run by now: what was saved only on disk is on Ed,
          // and Ed's newer files are on disk, so clean buffers read again.
          hold?.saver.synced();
          hold?.saver.forgetClean((p) => p === shown);
          setRemote((n) => n + 1);
        }
        break;
      case "files":
        setPaths(payload.files);
        setNamed((m) => new Map([...m].filter(([, copy]) => payload.files.includes(copy))));
        hold?.saver.forgetClean((p) => payload.files.includes(p));
        break;
      case "conflict":
        setNamed((m) => new Map(m).set(payload.path, payload.serverPath));
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

  const features = info?.features;
  const offline = edUnavailableReason(connection);
  const canRun = !!features?.run && info?.runCommand != null;
  const drawerTabs: DrawerTab[] = [];
  if (features?.terminal) drawerTabs.push("terminal");
  if (canRun) drawerTabs.push("run");
  const [drawerTab, setDrawerTab] = useState<DrawerTab>("terminal");
  const shownDrawerTab = drawerTabs.includes(drawerTab) ? drawerTab : drawerTabs[0];
  const terminal = useTerminalSession(
    challengeId,
    shownDrawerTab === "terminal" && !drawer.collapsed,
    connection.state === "online",
  );
  const run = useRunSession(challengeId, hold?.saver ?? null);
  const startRun = () => {
    setDrawerTab("run");
    drawer.setCollapsed(false);
    void run.start();
  };

  const [sideTab, setSideTab] = useState<SideTab>("description");
  const [marked, setMarked] = useState<Submission | null>(null);
  const mark = useMarkFlow({
    challengeId,
    saver: hold?.saver ?? null,
    confirmSubmit: !!features?.confirmSubmit,
    dueAt: info?.dueAt ?? null,
    onMarked: (submission) => {
      setMarked(submission);
      setSideTab("feedback");
    },
  });

  /** Rust's `files` follows a resolve; until then the copy is gone here too. */
  const resolved = (path: string) => {
    const copy = listing?.conflicts.get(path);
    setNamed((m) => new Map([...m].filter(([p]) => p !== path)));
    if (copy != null) setPaths((ps) => ps?.filter((p) => p !== copy) ?? ps);
  };

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
          <DescriptionColumn
            challengeId={challengeId}
            brief={brief}
            files={files}
            feedback={!!features && (features.feedback || features.mark)}
            tab={sideTab}
            onTab={setSideTab}
            marked={marked}
            openWorkspaceFile={(path) => {
              if (!tabs?.includes(path)) return false;
              select(path);
              return true;
            }}
          />
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
            {shown != null && tabs && (
              <PillTabs
                scroll
                tabs={tabs.map((p) => ({
                  value: p,
                  label: listing?.conflicts.has(p) ? (
                    <span className="inline-flex items-center gap-1">
                      {p}
                      <span
                        className="size-1.5 rounded-full bg-warning"
                        title="Ed has a different version of this file"
                      />
                    </span>
                  ) : (
                    p
                  ),
                }))}
                value={shown}
                onChange={select}
              />
            )}
          </div>
          {!openError && <WorkspaceStatusLine connection={connection} save={save} />}
          {canRun && <RunButton phase={run.phase} offline={offline} onRun={startRun} onStop={run.stop} />}
          {features?.mark && <MarkButton step={mark.state.step} offline={offline} onClick={() => void mark.start()} />}
        </div>
        {(openError || save.state === "error" || notice || run.error || mark.error) && (
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
            {run.error && (
              <Alert variant="destructive">
                <AlertTitle className="text-xs">Run failed</AlertTitle>
                <AlertDescription className="text-xs">{run.error}</AlertDescription>
              </Alert>
            )}
            {mark.error && (
              <Alert variant="destructive">
                <AlertTitle className="text-xs">{mark.error.title}</AlertTitle>
                <AlertDescription className="text-xs">{mark.error.message}</AlertDescription>
              </Alert>
            )}
          </div>
        )}
        <div className="min-h-0 flex-1">
          {hold && shown != null ? (
            <ConflictFrame
              key={shown}
              challengeId={challengeId}
              saver={hold.saver}
              path={shown}
              serverPath={listing?.conflicts.get(shown) ?? null}
              offline={offline}
              onResolved={resolved}
            >
              {(locked) => (
                <WorkspaceEditor
                  challengeId={challengeId}
                  saver={hold.saver}
                  path={shown}
                  remote={remote}
                  readOnly={locked}
                  memory={{
                    read: () => memories.current.get(shown),
                    write: (json) => memories.current.set(shown, json),
                  }}
                />
              )}
            </ConflictFrame>
          ) : tabs === null ? (
            !openError && <LoadingFill />
          ) : (
            tabs.length === 0 && <p className="px-4 py-3 text-sm text-muted-foreground">This workspace has no files.</p>
          )}
        </div>
        {shownDrawerTab && (
          <TerminalDrawer
            panel={drawer}
            tabs={drawerTabs}
            tab={shownDrawerTab}
            onTab={setDrawerTab}
            terminal={terminal}
            run={run}
            offline={offline}
          />
        )}
      </section>
      <MarkDialog
        prompts={mark.state.step === "confirm" ? mark.state.prompts : null}
        dueAt={mark.state.step === "confirm" ? mark.state.dueAt : null}
        onConfirm={mark.confirm}
        onCancel={mark.cancel}
      />
    </div>
  );
}
