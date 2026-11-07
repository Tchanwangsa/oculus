export interface DiagnosticEvent {
  at: number;
  name: string;
  fields: Record<string, string | number | boolean | null>;
}
export type DiagnosticMessage =
  | { type: 'init'; port: number; token: string; hostname: string; session: string; shareId: string }
  | { type: 'heartbeat'; visible: boolean; events: DiagnosticEvent[]; dropped: number }
  | { type: 'begin'; id: number; name: string; at: number }
  | { type: 'finish'; id: number };

/** Worker-side state is independent of the page's event loop. */
export class StallWatch {
  lastBeat = 0;
  visible = false;
  stalled = false;
  readonly active = new Map<number, { name: string; at: number }>();
  heartbeat(now: number, visible: boolean): DiagnosticEvent | null {
    const recovered = this.stalled ? { at: now, name: 'main-thread.resumed', fields: { durationMs: now - this.lastBeat } } : null;
    this.lastBeat = now;
    this.visible = visible;
    this.stalled = false;
    return recovered;
  }
  check(now: number): DiagnosticEvent | null {
    if (!this.visible || !this.lastBeat || this.stalled || now - this.lastBeat < 1000) return null;
    this.stalled = true;
    return { at: now, name: 'main-thread.unresponsive', fields: {
      lastHeartbeatAt: this.lastBeat, gapMs: now - this.lastBeat,
      active: JSON.stringify([...this.active.values()].slice(-12)),
    } };
  }
}

export function redactDiagnosticText(text: string, secrets: readonly string[]): string {
  let clean = text;
  for (const secret of secrets) if (secret) clean = clean.split(secret).join('[redacted]');
  return clean.replace(/data:[^\s"']+/g, '[data-url]').replace(/https?:\/\/[^/\s]+/g, '[origin]').slice(0, 3000);
}
