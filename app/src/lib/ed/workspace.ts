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
  | { challengeId: number; kind: "error"; message: string }
  /** Both sides changed `path`: ours stays, Ed's is written beside it at
   *  `serverPath` (`lib/ed/conflicts.ts`). */
  | { challengeId: number; kind: "conflict"; path: string; serverPath: string }
  /** Base64 PTY output of terminal `id`. */
  | { challengeId: number; kind: "term"; id: string; data: string }
  | { challengeId: number; kind: "termState"; id: string; state: "running" | "exited" }
  /** Base64 output of a run; stdout and stderr arrive merged, through a PTY. */
  | { challengeId: number; kind: "run"; runId: string; data: string }
  /** `message`: Ed's reason when there is one ("No resources available", a
   *  failed build's output). */
  | {
      challengeId: number;
      kind: "runExit";
      runId: string;
      code: number | null;
      signal: string | null;
      message: string | null;
    };

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

/** Ends a conflict: `mine` uploads the library copy, `ed` replaces it with
 *  Ed's; either way the server copy goes and a `files` event follows. */
export const resolveConflict = (challengeId: number, path: string, keep: "mine" | "ed") =>
  invoke<void>("ed_workspace_resolve", { challengeId, path, keep });

export const deleteWorkspaceFile = (challengeId: number, path: string) =>
  invoke<void>("ed_workspace_delete", { challengeId, path });

/** `resumed`: a terminal that persisted on Ed was re-attached, not started. */
export interface TerminalOpened {
  id: string;
  resumed: boolean;
}

/** Opens Ed's terminal (bash) at this size, or re-attaches a persisting one. */
export const openTerminal = (challengeId: number, cols: number, rows: number) =>
  invoke<TerminalOpened>("ed_terminal_open", { challengeId, cols, rows });

/** `data`: UTF-8 text as typed into xterm. */
export const terminalInput = (challengeId: number, id: string, data: string) =>
  invoke<void>("ed_terminal_input", { challengeId, id, data });

export const resizeTerminal = (challengeId: number, id: string, cols: number, rows: number) =>
  invoke<void>("ed_terminal_resize", { challengeId, id, cols, rows });

export const killTerminal = (challengeId: number, id: string) =>
  invoke<void>("ed_terminal_kill", { challengeId, id });

/** Runs the challenge's `run_command` on Ed's latest commit; the caller
 *  flushes pending saves first. Output streams as `run` events. */
export const runWorkspace = (challengeId: number, cols: number, rows: number) =>
  invoke<{ runId: string }>("ed_workspace_run", { challengeId, cols, rows });

export const runInput = (challengeId: number, runId: string, data: string) =>
  invoke<void>("ed_run_input", { challengeId, runId, data });

export const resizeRun = (challengeId: number, runId: string, cols: number, rows: number) =>
  invoke<void>("ed_run_resize", { challengeId, runId, cols, rows });

export const stopRun = (challengeId: number, runId: string) =>
  invoke<void>("ed_run_stop", { challengeId, runId });

/** One testcase of a marked submission. `output` is its run's output;
 *  `diffs` are its `check_diff` checks' lines, each a list of `[sign, text]`
 *  runs: `-` observed, `+` expected, `=` the same, `mn` a missing newline. */
export interface SubmissionTestcase {
  name: string;
  description: string | null;
  hidden: boolean;
  private: boolean;
  score: number | null;
  maxScore: number | null;
  passed: boolean;
  skip: boolean;
  feedback: string | null;
  output: string | null;
  diffs: Array<Array<[string, string]>>;
}

export interface Submission {
  id: number;
  status: string;
  createdAt: string;
  markedAt: string | null;
  testcasePassCount: number | null;
  testcaseTotalCount: number | null;
  feedback: string | null;
  result: {
    ok: boolean;
    passed: boolean;
    feedback: string | null;
    build: { ok: boolean; output: string } | null;
    testcases: SubmissionTestcase[];
  } | null;
}

/** The student's live standing on the challenge (`GET /challenges/<ch>`). */
export interface ChallengeStanding {
  score: number | null;
  status: string | null;
  /** Null when attempts are unlimited. */
  attemptsRemaining: number | null;
  dueAt: string | null;
}

/** Submits the workspace for marking: a real submission that counts on Ed.
 *  Only ever from the Mark button, after its prompts. */
export const markWorkspace = (challengeId: number) =>
  invoke<Submission>("ed_workspace_mark", { challengeId });

/** The submissions this install made, newest first. */
export const workspaceSubmissions = (challengeId: number) =>
  invoke<Submission[]>("ed_workspace_submissions", { challengeId });

export const challengeStanding = (challengeId: number) =>
  invoke<ChallengeStanding>("ed_challenge_standing", { challengeId });
