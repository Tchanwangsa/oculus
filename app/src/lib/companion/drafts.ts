import type { CanvasTransaction } from "@/lib/canvas/collaboration";
import type { DraftStorage } from "./env";
import type { ShareOp } from "./protocol";

/**
 * What survives a reload of a shared-canvas tab, per origin and share id: the
 * client's identity and seq counter, the op in flight (resent unchanged, so the
 * server can answer a replay), the unconfirmed edits as a transaction against
 * the shared canvas, and the draft drawing for download. Within the same epoch
 * the pending edits are rebased onto the fresh snapshot; from another epoch the
 * draft is only offered for download, never replayed.
 */
export interface ShareDraftRecord {
  v: 1;
  epoch: string;
  clientId: string;
  /** The lowest seq this client may still use: past the op in flight, whose fate may be unknown. */
  seqFloor: number;
  inflight: ShareOp | null;
  pending: CanvasTransaction | null;
  /** Edits relative to the sent operation's result; null includes an unchanged result. */
  afterInflight?: CanvasTransaction | null;
  /** A revision that already contains the sent op, even if its HTTP reply is pending. */
  inflightRevision?: number;
  /** A kept conflict stays an explicit choice across reloads. */
  conflict?: string;
  /** The serialized local drawing, while it holds unconfirmed edits. */
  draft: string | null;
  savedAt: number;
}

export const draftKey = (shareId: string) => `oculus:companion:${shareId}`;

export async function readDraft(storage: DraftStorage | null, shareId: string): Promise<ShareDraftRecord | null> {
  let text: string | null = null;
  try { text = await storage?.getItem(draftKey(shareId)) ?? null; } catch { return null; }
  if (!text) return null;
  try {
    const value = JSON.parse(text) as ShareDraftRecord;
    if (value?.v !== 1 || typeof value.epoch !== "string" || typeof value.clientId !== "string" || !Number.isSafeInteger(value.seqFloor) || value.seqFloor < 1) return null;
    const inflight = value.inflight && value.inflight.clientId === value.clientId && value.inflight.epoch === value.epoch && Number.isSafeInteger(value.inflight.seq) && typeof value.inflight.opId === "string" ? value.inflight : null;
    return {
      v: 1, epoch: value.epoch, clientId: value.clientId, seqFloor: value.seqFloor, inflight,
      pending: value.pending && Array.isArray(value.pending.edits) ? value.pending : null,
      ...(value.afterInflight === null || (value.afterInflight && Array.isArray(value.afterInflight.edits)) ? { afterInflight: value.afterInflight } : {}),
      ...(Number.isSafeInteger(value.inflightRevision) && value.inflightRevision! >= 0 ? { inflightRevision: value.inflightRevision } : {}),
      ...(typeof value.conflict === "string" ? { conflict: value.conflict } : {}),
      draft: typeof value.draft === "string" ? value.draft : null,
      savedAt: typeof value.savedAt === "number" ? value.savedAt : 0,
    };
  } catch { return null; }
}

/** Stores the complete record, including embedded pages; quota failures stay explicit. */
export async function writeDraft(storage: DraftStorage | null, shareId: string, record: ShareDraftRecord): Promise<boolean> {
  if (!storage) return false;
  try { await storage.setItem(draftKey(shareId), JSON.stringify(record)); return true; }
  catch { return false; }
}

/** A draft from an earlier epoch of this link, kept for download until dismissed. */
export interface ShareRecovery { epoch: string; draft: string; savedAt: number }
const recoveryKey = (shareId: string) => `${draftKey(shareId)}:recovery`;

export async function readRecovery(storage: DraftStorage | null, shareId: string): Promise<ShareRecovery | null> {
  try {
    const value = JSON.parse(await storage?.getItem(recoveryKey(shareId)) ?? "null") as ShareRecovery | null;
    return value && typeof value.epoch === "string" && typeof value.draft === "string" ? { epoch: value.epoch, draft: value.draft, savedAt: Number(value.savedAt) || 0 } : null;
  } catch { return null; }
}

export async function writeRecovery(storage: DraftStorage | null, shareId: string, recovery: ShareRecovery): Promise<boolean> {
  try { await storage?.setItem(recoveryKey(shareId), JSON.stringify(recovery)); return !!storage; } catch { return false; }
}

export async function clearRecovery(storage: DraftStorage | null, shareId: string): Promise<void> {
  try { await storage?.removeItem(recoveryKey(shareId)); } catch { /* storage unavailable */ }
}
