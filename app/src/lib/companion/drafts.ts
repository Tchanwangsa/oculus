import { measureWhiteboard, measureWhiteboardAsync } from "@/lib/whiteboard/diagnostics";
import type { WhiteboardTransaction } from "@/lib/whiteboard/collaboration";
import type { DraftStorage } from "./env";
import type { ShareOp } from "./protocol";

/**
 * What survives a reload of a shared-whiteboard tab, per origin and share id: the
 * client's identity and seq counter, the op in flight (resent unchanged, so the
 * server can answer a replay), the unconfirmed edits as a transaction against
 * the shared whiteboard, and the draft drawing for download. Within the same epoch
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
  pending: WhiteboardTransaction | null;
  /** Edits relative to the sent operation's result; null includes an unchanged result. */
  afterInflight?: WhiteboardTransaction | null;
  /** A revision that already contains the sent op, even if its HTTP reply is pending. */
  inflightRevision?: number;
  /** A kept conflict stays an explicit choice across reloads. */
  conflict?: string;
  /** The serialized local drawing, while it holds unconfirmed edits. */
  draft: string | null;
  savedAt: number;
  /** Timestamp of the complete drawing, which can trail the operation record. */
  draftSavedAt?: number;
}

export const draftKey = (shareId: string) => `oculus:companion:${shareId}`;

export const drawingDraftKey = (shareId: string) => `${draftKey(shareId)}:draft`;
export type ShareOpRecord = Omit<ShareDraftRecord, "draft" | "draftSavedAt">;

export async function readDraft(storage: DraftStorage | null, shareId: string): Promise<ShareDraftRecord | null> {
  let text: string | null = null;
  try { text = await storage?.getItem(draftKey(shareId)) ?? null; } catch { return null; }
  if (!text) return null;
  try {
    const value = JSON.parse(text) as ShareDraftRecord;
    if (value?.v !== 1 || typeof value.epoch !== "string" || typeof value.clientId !== "string" || !Number.isSafeInteger(value.seqFloor) || value.seqFloor < 1) return null;
    const inflight = value.inflight && value.inflight.clientId === value.clientId && value.inflight.epoch === value.epoch && Number.isSafeInteger(value.inflight.seq) && typeof value.inflight.opId === "string" ? value.inflight : null;
    let draft = typeof value.draft === "string" ? value.draft : null;
    let draftSavedAt = draft ? value.savedAt : 0;
    try {
      const drawing = JSON.parse(await storage?.getItem(drawingDraftKey(shareId)) ?? "null");
      if (drawing?.epoch === value.epoch && drawing.draft && typeof drawing.draft === "object" && (value.pending || inflight || value.conflict || drawing.savedAt >= value.savedAt)) {
        draft = JSON.stringify(drawing.draft);
        draftSavedAt = Number(drawing.savedAt) || 0;
      }
    } catch { /* the inline draft remains usable when separate storage is unavailable */ }
    return {
      v: 1, epoch: value.epoch, clientId: value.clientId, seqFloor: value.seqFloor, inflight,
      pending: value.pending && Array.isArray(value.pending.edits) ? value.pending : null,
      ...(value.afterInflight === null || (value.afterInflight && Array.isArray(value.afterInflight.edits)) ? { afterInflight: value.afterInflight } : {}),
      ...(Number.isSafeInteger(value.inflightRevision) && value.inflightRevision! >= 0 ? { inflightRevision: value.inflightRevision } : {}),
      ...(typeof value.conflict === "string" ? { conflict: value.conflict } : {}),
      draft, draftSavedAt,
      savedAt: typeof value.savedAt === "number" ? value.savedAt : 0,
    };
  } catch { return null; }
}

/** Stores only operations; complete drawings have their own key. */
export async function writeDraft(storage: DraftStorage | null, shareId: string, record: ShareOpRecord): Promise<boolean> {
  if (!storage) return false;
  try { await measureWhiteboardAsync("storage.operations", () => storage.setItem(draftKey(shareId), measureWhiteboard("storage.serialize-operations", () => JSON.stringify(record)))); return true; }
  catch { return false; }
}

/** The drawing is raw compact JSON inside the envelope, without escaping it again. */
export async function writeDrawingDraft(storage: DraftStorage | null, shareId: string, record: ShareRecovery | null): Promise<boolean> {
  if (!storage) return false;
  try {
    if (record) await measureWhiteboardAsync("storage.drawing", () => storage.setItem(drawingDraftKey(shareId), `{"epoch":${JSON.stringify(record.epoch)},"savedAt":${record.savedAt},"draft":${record.draft}}`));
    else await storage.removeItem(drawingDraftKey(shareId));
    return true;
  } catch { return false; }
}

/** One active write and one replaceable pending value; callers wait for their version or newer. */
export class LatestWriter<T> {
  private version = 0;
  private pending: { version: number; value: T } | null = null;
  private running = false;
  private waiters: { version: number; resolve: () => void }[] = [];

  constructor(private readonly write: (value: T) => Promise<boolean>, private readonly finished: (kept: boolean) => void) {}

  store(value: T): Promise<void> {
    const version = ++this.version;
    this.pending = { version, value };
    const promise = new Promise<void>(resolve => this.waiters.push({ version, resolve }));
    if (!this.running) void this.drain();
    return promise;
  }

  private async drain(): Promise<void> {
    this.running = true;
    while (this.pending) {
      const { version, value } = this.pending;
      this.pending = null;
      let kept = false;
      try { kept = await this.write(value); } catch { /* a failed write still settles its callers */ }
      if (version === this.version) this.finished(kept);
      const done = this.waiters.filter(waiter => waiter.version <= version);
      this.waiters = this.waiters.filter(waiter => waiter.version > version);
      for (const waiter of done) waiter.resolve();
    }
    this.running = false;
  }
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
