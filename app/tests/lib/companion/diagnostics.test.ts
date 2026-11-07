import { expect, test } from 'bun:test';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { installCanvasTrace, measureCanvas, traceCanvas } from '@/lib/canvas/diagnostics';
import { redactDiagnosticText, StallWatch, type DiagnosticEvent } from '@/companion/diagnostics/protocol';
import { allowedOrigin, logLines, MAX_BODY, readBatch } from '../../../scripts/companion-diagnostics';

test('diagnostics cannot change a handler result or exception, even when the receiver fails', () => {
  const stop = installCanvasTrace({ event() { throw new Error('offline'); }, begin() { throw new Error('offline'); } });
  try {
    traceCanvas('input.test');
    expect(measureCanvas('handler.test', () => 42)).toBe(42);
    expect(() => measureCanvas('handler.test', () => { throw new Error('original'); })).toThrow('original');
  } finally { stop(); }
  expect(measureCanvas('disabled', () => 7)).toBe(7);
});

test('trace strings redact sharing and receiver secrets and embedded image data', () => {
  const text = redactDiagnosticText('http://192.168.1.2:43821/c/assets/canvas.js#share-secret receiver-token data:image/png;base64,AAAA', ['share-secret', 'receiver-token']);
  expect(text).not.toContain('share-secret');
  expect(text).not.toContain('receiver-token');
  expect(text).not.toContain('192.168.1.2');
  expect(text).not.toContain('AAAA');
  expect(text).toContain('canvas.js');
  expect(redactDiagnosticText('x'.repeat(5000), [])).toHaveLength(3000);
});

test('worker watchdog identifies the active operation once, then reports recovery', () => {
  const watch = new StallWatch();
  watch.heartbeat(1000, true);
  watch.active.set(1, { name: 'canvas.paint', at: 1100 });
  expect(watch.check(1999)).toBeNull();
  expect(watch.check(2400)).toMatchObject({ name: 'main-thread.unresponsive', fields: { gapMs: 1400 } });
  expect(watch.check(3000)).toBeNull();
  expect(watch.heartbeat(4200, true)).toMatchObject({ name: 'main-thread.resumed', fields: { durationMs: 3200 } });
  watch.heartbeat(4300, false);
  expect(watch.check(10000)).toBeNull();
});

test('collector accepts the sharing origin on the same host, not unrelated sites or ports', () => {
  expect(allowedOrigin('http://192.168.1.9:43821', '192.168.1.9:43841')).toBe('http://192.168.1.9:43821');
  expect(allowedOrigin('http://localhost:43840', 'localhost:43841')).toBe('http://localhost:43840');
  expect(allowedOrigin('http://evil.example:43821', 'localhost:43841')).toBeNull();
  expect(allowedOrigin('http://localhost:8000', 'localhost:43841')).toBeNull();
  expect(allowedOrigin(null, 'localhost:43841')).toBeNull();
});

test('collector validates fields and bounds streaming request bodies without a content-length', async () => {
  const lines = logLines({ session: 'session-1', shareId: 'share-1', events: [{ at: 1234, name: 'input.pointerdown', fields: { x: 12, canvas: { elements: ['private'] }, content: ['private'], label: 'Pen' } }] }, 5678);
  expect(JSON.parse(lines)).toMatchObject({ at: 1234, receivedAt: 5678, fields: { x: 12, label: 'Pen' } });
  expect(lines).not.toContain('private');
  expect(() => logLines({ session: '../bad', events: [] })).toThrow();
  const request = new Request('http://localhost/events', { method: 'POST', body: new ReadableStream({ start(controller) { controller.enqueue(new Uint8Array(MAX_BODY + 1)); controller.close(); } }), duplex: 'half' } as RequestInit);
  await expect(readBatch(request)).rejects.toThrow('too-large');
});

test('real diagnostic worker reports a stalled main thread and its active phase to HTTP', async () => {
  const received: DiagnosticEvent[] = [];
  let report!: () => void;
  const reported = new Promise<void>(resolve => { report = resolve; });
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(request) {
    const batch = await request.json() as { events: DiagnosticEvent[] };
    received.push(...batch.events);
    if (received.some(event => event.name === 'main-thread.unresponsive')) report();
    return Response.json({ ok: true });
  } });
  const worker = new Worker(new URL('../../../src/companion/diagnostics/worker.ts', import.meta.url).href);
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    worker.postMessage({ type: 'init', port: server.port, token: 'test-token', hostname: '127.0.0.1', session: 'test-worker', shareId: 'share' });
    worker.postMessage({ type: 'begin', id: 1, name: 'canvas.paint', at: Date.now() });
    worker.postMessage({ type: 'heartbeat', visible: true, events: [], dropped: 0 });
    await Promise.race([reported, new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error('No worker stall report')), 4500); })]);
    expect(received.find(event => event.name === 'main-thread.unresponsive')?.fields.active).toContain('canvas.paint');
  } finally { clearTimeout(timeout); worker.terminate(); await server.stop(true); }
}, 6000);

test('real receiver persists an authenticated batch, refuses foreign origins and cleans up config', async () => {
  const root = await mkdtemp(join(tmpdir(), 'oculus-input-diagnostics-'));
  const proc = Bun.spawn(['bun', resolve('scripts/companion-diagnostics.ts'), root], { stdout: 'pipe', stderr: 'pipe', env: { ...process.env, OCULUS_DIAGNOSTICS_PORT: '0' } });
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    const reader = proc.stdout.getReader();
    await Promise.race([reader.read(), new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error('Receiver did not start')), 4000); })]);
    clearTimeout(timeout);
    const configPath = join(root, 'app/dist/companion/assets/diagnostics-config.json');
    const config = JSON.parse(await readFile(configPath, 'utf8'));
    const url = `http://127.0.0.1:${config.port}/events`;
    const headers = { Origin: 'http://127.0.0.1:43821', Authorization: `Bearer ${config.token}`, 'Content-Type': 'application/json' };
    const body = JSON.stringify({ session: 'test-session', shareId: 'test-share', events: [{ at: 123, name: 'input.click', fields: { control: 'Pen' } }] });
    expect((await fetch(url, { method: 'POST', headers: { ...headers, Origin: 'http://other.example:43821' }, body })).status).toBe(403);
    expect((await fetch(url, { method: 'POST', headers: { ...headers, Authorization: 'Bearer wrong' }, body })).status).toBe(401);
    expect((await fetch(url, { method: 'POST', headers, body })).status).toBe(200);
    const lines = await readFile(join(root, 'data/logs/companion-input.jsonl'), 'utf8');
    expect(JSON.parse(lines)).toMatchObject({ at: 123, name: 'input.click', fields: { control: 'Pen' } });
    expect(lines).not.toContain(config.token);
    proc.kill('SIGTERM');
    await proc.exited;
    expect(await readFile(configPath, 'utf8').catch(() => null)).toBeNull();
  } finally { clearTimeout(timeout); proc.kill(); await proc.exited; await rm(root, { recursive: true, force: true }); }
}, 8000);
