import { invoke } from "@tauri-apps/api/core";

/**
 * An Ed challenge's workspace, as Rust's `ed_workspace_*` commands keep it
 * (docs/viewers.md). The library copy under the slide's folder is the working
 * copy: every save writes it first, then uploads to Ed while the socket is up.
 * Paths are workspace-relative ("tally.py", "src/a.py").
 */

export type WorkspaceState = "connecting" | "online" | "offline";

export interface WorkspaceStatus {
  state: WorkspaceState;
  reason: string | null;
}

export interface WorkspaceFeatures {
  run: boolean;
  mark: boolean;
  terminal: boolean;
  feedback: boolean;
  confirmSubmit: boolean;
}

export interface WorkspaceInfo {
  challengeId: number;
  /** Data-dir-relative: `courses/<CODE>/lessons/<id>-<slug>/<NN>-<slide-slug>`. */
  slideDir: string;
  /** Sorted, read from disk. */
  files: string[];
  /** Ed's `run_command`; null when it has none. */
  runCommand: string | null;
  features: WorkspaceFeatures;
  dueAt: string | null;
  /** `connecting` right after open; the socket reports the rest as events. */
  status: WorkspaceStatus;
}

/** Where a save landed: uploaded to Ed (`hash` is its commit), or kept on
 *  disk only while offline. */
export interface WorkspaceSaved {
  saved: "ed" | "local";
  hash: string | null;
}

/** The `ed-workspace` event's payload, for every open workspace. */
export type WorkspaceEvent =
  | { challengeId: number; kind: "status"; status: WorkspaceStatus }
  /** `ours: false` — another client (Ed's web app) saved. */
  | { challengeId: number; kind: "commit"; hash: string; ours: boolean }
  | { challengeId: number; kind: "files"; files: string[] }
  | { challengeId: number; kind: "error"; message: string };

export const WORKSPACE_EVENT = "ed-workspace";

/** Opens the session; returns at once and connects in the background. */
export const openWorkspace = (challengeId: number) =>
  invoke<WorkspaceInfo>("ed_workspace_open", { challengeId });

/** Idempotent. */
export const closeWorkspace = (challengeId: number) =>
  invoke<void>("ed_workspace_close", { challengeId });

export const readWorkspaceFile = (challengeId: number, path: string) =>
  invoke<string>("ed_workspace_read", { challengeId, path });

/** Writes the library file, then uploads it when online. A failed upload
 *  while online rejects, though the disk write still happened. */
export const saveWorkspaceFile = (challengeId: number, path: string, text: string) =>
  invoke<WorkspaceSaved>("ed_workspace_save", { challengeId, path, text });

export const deleteWorkspaceFile = (challengeId: number, path: string) =>
  invoke<void>("ed_workspace_delete", { challengeId, path });
