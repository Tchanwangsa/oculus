import { expect, test } from 'bun:test';
import { InputHealth } from '@/companion/diagnostics/health';

test('health reports frame and timer progress even when no inputs arrive', () => {
  const health = new InputHealth(100);
  health.frame(16); health.frame(18); health.frame(17);
  health.timer(250); health.timer(261);
  expect(health.report(5099, true)).toBeNull();
  expect(health.report(5100, true)).toEqual({ visible: true, intervalMs: 5000, frames: 3, maxFrameGapMs: 18, maxTimerGapMs: 261, contacts: '[]', captures: '[]' });
  expect(health.report(10100, true)).toMatchObject({ frames: 0, maxFrameGapMs: 0, maxTimerGapMs: 0 });
});

test('contact release and pointer-capture release are tracked independently', () => {
  const health = new InputHealth(0);
  health.pointer('pointerdown', -20, 'pen', 1234);
  health.pointer('gotpointercapture', -20, 'pen', 1235);
  health.pointer('pointerup', -20, 'pen', 1300);
  expect(health.report(5000, true)).toMatchObject({ contacts: '[]', captures: '[-20]' });
  health.pointer('lostpointercapture', -20, 'pen', 1301);
  health.pointer('pointerdown', 2, 'touch', 1400);
  health.pointer('pointerdown', 3, 'touch', 1401);
  health.pointer('pointercancel', 2, 'touch', 1402);
  const next = health.report(10000, true)!;
  expect(JSON.parse(next.contacts as string)).toEqual([{ pointerId: 3, pointerType: 'touch', startedAt: 1401 }]);
  expect(next.captures).toBe('[]');
});
