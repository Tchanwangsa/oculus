import type { TraceFields } from '@/lib/whiteboard/diagnostics';

/** Positive liveness records distinguish a quiet page from a page losing input. */
export class InputHealth {
  private readonly contacts = new Map<number, { pointerId: number; pointerType: string; startedAt: number }>();
  private readonly captures = new Set<number>();
  private frames = 0;
  private maxFrameGapMs = 0;
  private maxTimerGapMs = 0;

  constructor(private lastReport: number) {}

  pointer(type: string, pointerId: number, pointerType: string, at: number): void {
    if (!Number.isFinite(pointerId)) return;
    if (type === 'pointerdown' && (this.contacts.size < 32 || this.contacts.has(pointerId))) this.contacts.set(pointerId, { pointerId, pointerType, startedAt: at });
    if (type === 'pointerup' || type === 'pointercancel') this.contacts.delete(pointerId);
    if (type === 'gotpointercapture' && this.captures.size < 32) this.captures.add(pointerId);
    if (type === 'lostpointercapture') this.captures.delete(pointerId);
  }

  frame(gapMs: number): void { this.frames++; this.maxFrameGapMs = Math.max(this.maxFrameGapMs, gapMs); }
  timer(gapMs: number): void { this.maxTimerGapMs = Math.max(this.maxTimerGapMs, gapMs); }

  active(): TraceFields {
    return { contacts: JSON.stringify([...this.contacts.values()]), captures: JSON.stringify([...this.captures]) };
  }

  report(now: number, visible: boolean): TraceFields | null {
    if (now - this.lastReport < 5000) return null;
    const fields = {
      visible, intervalMs: Math.round(now - this.lastReport), frames: this.frames,
      maxFrameGapMs: Math.round(this.maxFrameGapMs), maxTimerGapMs: Math.round(this.maxTimerGapMs),
      ...this.active(),
    };
    this.lastReport = now;
    this.frames = this.maxFrameGapMs = this.maxTimerGapMs = 0;
    return fields;
  }
}
