import type { CanvasTransaction } from "@/lib/canvas/collaboration";

/** Wire protocol v1 between a paired browser and the Mac's companion server
 * (docs/device-sharing.md). Only commits write; the socket only announces. */

/** One commit: `seq` increases per `clientId`; a retry resends the same op unchanged. */
export interface ShareOp { clientId: string; seq: number; opId: string; epoch: string; transaction: CanvasTransaction }
/** A replayed ack carries the original revision and no transaction. */
export interface ShareAck { epoch: string; revision: number; seq: number; opId: string; replayed: boolean; transaction: CanvasTransaction | null }
export interface ShareChange { epoch: string; revision: number; clientId: string | null; transaction: CanvasTransaction }
export interface ShareSnapshot { epoch: string; revision: number; text: string }

export type SharePhase = "live" | "paused" | "ended";
export type ServerFrame =
  | { type: "welcome"; epoch: string; revision: number; clients: number }
  | { type: "revision"; epoch: string; revision: number }
  | ({ type: "change" } & ShareChange)
  | { type: "presence"; clients: number }
  | { type: "state"; phase: SharePhase }
  | { type: "session-ended"; reason: string }
  | { type: "ping" };

const record = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const revision = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
/** `clients` is a count; a list of client entries counts as its length. */
const count = (value: unknown): number => Array.isArray(value) ? value.length : typeof value === "number" && Number.isFinite(value) && value >= 0 ? Math.floor(value) : 0;
const transaction = (value: unknown): value is CanvasTransaction => record(value) && Array.isArray(value.edits);

export function parseChange(value: unknown): ShareChange | null {
  if (!record(value) || typeof value.epoch !== "string" || !revision(value.revision) || !transaction(value.transaction)) return null;
  const clientId = typeof value.clientId === "string" ? value.clientId : null;
  return { epoch: value.epoch, revision: value.revision, clientId, transaction: value.transaction };
}

export function parseAck(value: unknown): ShareAck | null {
  if (!record(value) || typeof value.epoch !== "string" || !revision(value.revision) || typeof value.opId !== "string" || typeof value.seq !== "number") return null;
  const tx = value.transaction === null || value.transaction === undefined ? null : transaction(value.transaction) ? value.transaction : undefined;
  if (tx === undefined) return null;
  return { epoch: value.epoch, revision: value.revision, seq: value.seq, opId: value.opId, replayed: value.replayed === true, transaction: tx };
}

/** A frame the client understands, or null for anything else (ignored). */
export function parseFrame(data: unknown): ServerFrame | null {
  if (typeof data !== "string" || data.length > 1 << 20) return null;
  let value: unknown;
  try { value = JSON.parse(data); } catch { return null; }
  if (!record(value) || typeof value.type !== "string") return null;
  switch (value.type) {
    case "welcome":
      return typeof value.epoch === "string" && revision(value.revision) ? { type: "welcome", epoch: value.epoch, revision: value.revision, clients: count(value.clients) } : null;
    case "revision":
      return typeof value.epoch === "string" && revision(value.revision) ? { type: "revision", epoch: value.epoch, revision: value.revision } : null;
    case "change": {
      const change = parseChange(value);
      return change ? { type: "change", ...change } : null;
    }
    case "presence": return { type: "presence", clients: count(value.clients) };
    case "state":
      return value.phase === "live" || value.phase === "paused" || value.phase === "ended" ? { type: "state", phase: value.phase } : null;
    case "session-ended": return { type: "session-ended", reason: typeof value.reason === "string" ? value.reason : "" };
    case "ping": return { type: "ping" };
    default: return null;
  }
}
