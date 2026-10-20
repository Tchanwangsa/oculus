import {
  closeWorkspace,
  openWorkspace,
  saveWorkspaceFile,
  type WorkspaceInfo,
} from "@/lib/ed/workspace";
import { createWorkspaceSaver, type WorkspaceSaver } from "@/lib/ed/workspaceSaver";

/**
 * One open workspace per challenge, shared by every pane showing it, so two
 * panes on the same slide edit one set of buffers. The first hold opens the
 * session; the last release writes what is unsaved and then closes it, a
 * tick later, so React's mount–unmount–mount in development (or a pane
 * remounting) keeps the session rather than closing it under the new mount.
 */
interface Entry {
  holds: number;
  info: Promise<WorkspaceInfo>;
  saver: WorkspaceSaver;
}

export interface WorkspaceHold {
  info: Promise<WorkspaceInfo>;
  saver: WorkspaceSaver;
  release(): void;
}

const entries = new Map<number, Entry>();
/** Closes in flight; a new open waits for its challenge's, so it never
 *  lands before a close meant for the session before it. */
const closing = new Map<number, Promise<void>>();

export function holdWorkspace(challengeId: number): WorkspaceHold {
  let entry = entries.get(challengeId);
  const reused = entry != null;
  if (!entry) {
    const info = (closing.get(challengeId) ?? Promise.resolve()).then(() => openWorkspace(challengeId));
    // A failed open forgets the entry, so the next hold tries again.
    info.catch(() => {
      if (entries.get(challengeId) === entry) entries.delete(challengeId);
    });
    entry = {
      holds: 0,
      info,
      saver: createWorkspaceSaver({
        write: (path, text) => saveWorkspaceFile(challengeId, path, text).then((r) => r.saved),
      }),
    };
    entries.set(challengeId, entry);
  }
  const held = entry;
  held.holds += 1;
  let released = false;
  // The first answer is a snapshot (`connecting`); a pane mounting later has
  // missed the events since, so it asks again, which answers for the live session.
  const info = reused ? held.info.then(() => openWorkspace(challengeId)) : held.info;
  return {
    info,
    saver: held.saver,
    release() {
      if (released) return;
      released = true;
      held.holds -= 1;
      if (held.holds > 0) return;
      void held.saver.flush().finally(() => {
        setTimeout(() => {
          if (held.holds > 0 || entries.get(challengeId) !== held) return;
          // A write that failed still reached the library copy (only the
          // upload failed), so closing loses nothing on this Mac.
          entries.delete(challengeId);
          const closed = held.info
            .then(() => closeWorkspace(challengeId))
            .catch((e) => console.warn(`Ed workspace ${challengeId} not closed:`, e))
            .finally(() => {
              if (closing.get(challengeId) === closed) closing.delete(challengeId);
            });
          closing.set(challengeId, closed);
        }, 0);
      });
    },
  };
}

/** The saver of a challenge's open workspace, for a panel that replaces
 *  its files (Restore) without holding the workspace itself. */
export function heldSaver(challengeId: number): WorkspaceSaver | null {
  return entries.get(challengeId)?.saver ?? null;
}

/** Writes every open workspace's unsaved text now. */
export function flushWorkspaces(): Promise<void> {
  return Promise.all([...entries.values()].map((e) => e.saver.flush())).then(() => {});
}

// A reload or quit drops anything not yet written; best effort.
if (typeof window !== "undefined") {
  const flush = () => void flushWorkspaces();
  window.addEventListener("pagehide", flush);
  window.addEventListener("beforeunload", flush);
  import.meta.hot?.dispose(() => {
    window.removeEventListener("pagehide", flush);
    window.removeEventListener("beforeunload", flush);
  });
}
