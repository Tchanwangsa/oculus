import type { WhiteboardTransaction } from "./collaboration";
import type { WhiteboardViewport } from "./model";

/** What a save session sees of the native whiteboard authority while a whiteboard is
 * shared (docs/drawing-canvas.md, "A shared canvas has one writer"). Injected
 * through `WhiteboardSessionIO.authority`, so the session logic runs on fakes in tests;
 * `files.ts` holds the Tauri-backed one. */
export interface WhiteboardAuthorityIO<F> {
  /** Joins a native session that survived a webview reload; null means ordinary file saves are available. */
  existing?(file: F): Promise<WhiteboardAuthoritySnapshot | null>;
  /** Starts sharing from `text`, which must be the bytes on disk; a file that is already shared answers with its live snapshot. */
  open(file: F, text: string): Promise<WhiteboardAuthoritySnapshot>;
  commit(file: F, op: WhiteboardAuthorityOp): Promise<WhiteboardAuthorityAck>;
  snapshot(file: F): Promise<WhiteboardAuthoritySnapshot>;
  resolve(file: F, choice: WhiteboardExternalChoice): Promise<WhiteboardAuthoritySnapshot>;
  /** Moves the shared file; the authority updates its `files` row. */
  rename(file: F, title: string): Promise<F>;
  /** Deletes the shared file and ends its epoch. */
  remove(file: F): Promise<void>;
  /** Ends the epoch, saving a pending viewport first. */
  close(file: F): Promise<void>;
  /** Changes and lifecycle moves for one file, in revision order. */
  onRemote(fileId: number, listener: (event: WhiteboardAuthorityEvent) => void): () => void;
}

export type WhiteboardAuthorityCode =
  | "conflict-shape" | "conflict-order" | "conflict-background" | "invalid" | "stale-op"
  | "session-ended" | "too-large" | "external-edit" | "io";
const CODES: ReadonlySet<string> = new Set<WhiteboardAuthorityCode>([
  "conflict-shape", "conflict-order", "conflict-background", "invalid", "stale-op", "session-ended", "too-large", "external-edit", "io",
]);

/** Where sharing stands for one session; `off` saves through the whole-file write. */
export type WhiteboardAuthorityPhase = "off" | "attaching" | "live" | "paused" | "ended";
export type WhiteboardExternalChoice = "adopt-disk" | "keep-ours";

export interface WhiteboardAuthoritySnapshot { epoch: string; revision: number; text: string; relativePath: string; paused: boolean }
export interface WhiteboardAuthorityOp {
  clientId: string;
  seq: number;
  opId: string;
  transaction: WhiteboardTransaction;
  /** The desktop's view, saved as the file's initial view; never a content change. */
  viewport?: WhiteboardViewport;
  epoch?: string;
}
/** A replay carries the original revision and no transaction. */
export interface WhiteboardAuthorityAck { epoch: string; revision: number; seq: number; opId: string; replayed: boolean; transaction: WhiteboardTransaction | null }
export type WhiteboardAuthorityEvent =
  | { kind: "change"; fileId: number; epoch: string; revision: number; clientId: string | null; transaction: WhiteboardTransaction }
  | { kind: "state"; fileId: number; epoch: string; phase: "live" | "paused" | "ended" };

/** A refusal from the authority: nothing was applied and the op's `seq` is free again. */
export class WhiteboardAuthorityError extends Error {
  constructor(readonly code: WhiteboardAuthorityCode, message: string) { super(message); this.name = "WhiteboardAuthorityError"; }
}

/** The authority's code, or undefined for a failure that may or may not have reached it. */
export function authorityCode(error: unknown): WhiteboardAuthorityCode | undefined {
  const code = typeof error === "object" && error !== null ? (error as { code?: unknown }).code : undefined;
  return typeof code === "string" && CODES.has(code) ? code as WhiteboardAuthorityCode : undefined;
}

/** Native commands reject with a plain `{code, message}`. */
export function toAuthorityError(error: unknown): unknown {
  const code = authorityCode(error);
  if (!code || error instanceof WhiteboardAuthorityError) return error;
  return new WhiteboardAuthorityError(code, errorMessage(error));
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  const message = typeof error === "object" && error !== null ? (error as { message?: unknown }).message : undefined;
  return typeof message === "string" ? message : String(error);
}
