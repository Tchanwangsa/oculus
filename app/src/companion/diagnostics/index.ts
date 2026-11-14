import { installCanvasTrace, type TraceFields } from '@/lib/canvas/diagnostics';
import { InputHealth } from './health';
import { secureId } from '@/lib/canvas/ids';
import type { ShareCredentials } from '@/lib/companion/link';
import { redactDiagnosticText, type DiagnosticEvent, type DiagnosticMessage } from './protocol';

/** Only a receiver's same-origin config enables tracing; ordinary builds have no worker or timers. */
export async function startCompanionDiagnostics(credentials: ShareCredentials, page: 'drawing' | 'input-test' = 'drawing'): Promise<boolean> {
  try {
    const response = await fetch('/c/assets/diagnostics-config.json', { cache: 'no-store', credentials: 'omit' });
    if (!response.ok) return false;
    const config = await response.json() as { port?: number; token?: string };
    if (!Number.isSafeInteger(config.port) || config.port! < 1 || config.port! > 65535 || typeof config.token !== 'string' || config.token.length < 32) return false;
    attachDiagnostics(credentials, config as { port: number; token: string }, page);
    return true;
  } catch { return false; /* drawing works without diagnostics or worker support */ }
}

function attachDiagnostics(credentials: ShareCredentials, config: { port: number; token: string }, page: 'drawing' | 'input-test'): void {
  const worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
  const send = (message: DiagnosticMessage) => worker.postMessage(message);
  send({ type: 'init', ...config, hostname: location.hostname, session: secureId(), shareId: credentials.shareId });
  const sanitize = (text: string) => redactDiagnosticText(text, [credentials.secret, config.token]);
  const events: DiagnosticEvent[] = [];
  let dropped = 0, nextSpan = 0;
  const record = (name: string, fields: TraceFields = {}) => {
    const clean: TraceFields = {};
    for (const [key, value] of Object.entries(fields)) clean[key] = typeof value === 'string' ? sanitize(value) : value;
    events.push({ at: Date.now(), name, fields: clean });
    if (events.length > 200) { events.shift(); dropped++; }
  };
  installCanvasTrace({
    event: record,
    begin(name, fields) {
      const id = ++nextSpan, start = performance.now(), at = Date.now();
      send({ type: 'begin', id, name, at });
      return () => {
        const durationMs = performance.now() - start;
        send({ type: 'finish', id });
        if (durationMs >= (fields.asynchronous ? 250 : 32)) record('operation.slow', {
          ...fields, operation: name, startedAt: at, durationMs: Math.round(durationMs),
          stack: sanitize(new Error(`Slow ${name}`).stack ?? ''),
        });
      };
    },
  });
  const health = new InputHealth(performance.now());
  let moves = 0, wheels = 0, touchMoves = 0, gestureChanges = 0, lastMove = 0, lastTouchMove = 0;
  const lastGestureChange = [0, 0];
  const input = (event: Event, phase: string) => {
    const now = performance.now();
    if (event.type === 'pointermove') { moves++; if (now - lastMove < 150) return; lastMove = now; }
    if (event.type === 'touchmove') { touchMoves++; if (now - lastTouchMove < 150) return; lastTouchMove = now; }
    if (event.type === 'gesturechange') {
      if (phase === 'capture') gestureChanges++;
      const samplePhase = phase === 'capture' ? 0 : 1;
      if (now - lastGestureChange[samplePhase] < 150) return;
      lastGestureChange[samplePhase] = now;
    }
    if (event.type === 'wheel') { wheels++; if (now - lastMove < 150) return; lastMove = now; }
    const pointer = event as PointerEvent;
    const touch = event as TouchEvent;
    if (phase === 'capture' && event.type.includes('pointer')) health.pointer(event.type, pointer.pointerId, pointer.pointerType ?? '', Date.now());
    const target = event.target instanceof Element ? event.target : null;
    const control = target?.closest('button,[role="button"],canvas,[data-canvas-editor],[role="dialog"]');
    const editable = !!target?.closest('input,textarea,[contenteditable="true"]');
    const key = event as KeyboardEvent;
    record(`input.${event.type}`, {
      phase, scope: phase === 'capture' ? 'window' : 'document', target: control?.tagName ?? target?.tagName ?? 'unknown',
      control: control?.getAttribute('aria-label') ?? control?.getAttribute('title') ?? '',
      bodyPointerEvents: document.body.style.pointerEvents, canvasInert: !!document.querySelector('[data-canvas-editor]')?.closest('[inert]'),
      selectionType: event.type === 'selectionchange' ? document.getSelection()?.type ?? '' : '',
      disabled: !!control?.hasAttribute('disabled'), inert: !!target?.closest('[inert]'),
      touches: touch.touches?.length ?? 0, changedTouches: touch.changedTouches?.length ?? 0,
      ...(event.type.startsWith('gesture') ? { scale: (event as Event & { scale?: number }).scale ?? 1, rotation: (event as Event & { rotation?: number }).rotation ?? 0 } : {}),
      cancelable: event.cancelable, prevented: event.defaultPrevented, pointerType: pointer.pointerType ?? '', pointerId: pointer.pointerId ?? -1,
      buttons: pointer.buttons ?? 0, x: Math.round(pointer.clientX || 0), y: Math.round(pointer.clientY || 0),
      delayMs: Math.max(0, Math.round(event.timeStamp > 1e12 ? Date.now() - event.timeStamp : now - event.timeStamp)),
      key: event.type === 'keydown' ? editable ? 'editable' : /^[a-z0-9]$/i.test(key.key) ? (key.ctrlKey || key.metaKey || 'vhrodlapteszy'.includes(key.key.toLowerCase()) ? key.key : 'character') : key.key : '',
    });
  };
  for (const name of ['pointerdown', 'pointerup', 'pointercancel', 'pointermove', 'gotpointercapture', 'lostpointercapture', 'click', 'wheel', 'touchstart', 'touchmove', 'touchend', 'touchcancel', 'contextmenu', 'selectstart', 'selectionchange', 'dblclick', 'gesturestart', 'gesturechange', 'gestureend', 'keydown']) {
    window.addEventListener(name, event => input(event, 'capture'), { capture: true, passive: true });
  }
  for (const name of ['pointerdown', 'pointerup', 'click', 'touchstart', 'touchend', 'touchcancel', 'selectstart', 'contextmenu', 'dblclick', 'gesturestart', 'gesturechange', 'gestureend']) document.addEventListener(name, event => input(event, 'bubble'), { passive: true });
  window.addEventListener('error', event => record('javascript.error', { message: event.message, stack: event.error instanceof Error ? event.error.stack ?? '' : '', line: event.lineno, column: event.colno }));
  window.addEventListener('unhandledrejection', event => record('javascript.rejection', { message: event.reason instanceof Error ? event.reason.message : 'Unhandled rejection', stack: event.reason instanceof Error ? event.reason.stack ?? '' : '' }));
  for (const name of ['pageshow', 'pagehide', 'online', 'offline', 'focus', 'blur']) window.addEventListener(name, () => record(`page.${name}`));
  record('diagnostics.started', { userAgent: navigator.userAgent, timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone, offsetMinutes: new Date().getTimezoneOffset(), build: 'input-trace-v8', page, touchPolicy: page === 'input-test' ? 'probe-body-touch-defaults-blocked' : 'canvas-touch-defaults-blocked-at-body', gesturePolicy: page === 'input-test' ? 'browser-defaults' : 'canvas-native-gestures-blocked', width: innerWidth, height: innerHeight });
  let lastViewport = 0;
  const recordViewport = () => {
    const now = performance.now();
    if (now - lastViewport < 150) return;
    lastViewport = now;
    const view = window.visualViewport;
    record('page.viewport', { scale: view?.scale ?? 1, width: view?.width ?? innerWidth, height: view?.height ?? innerHeight, offsetLeft: view?.offsetLeft ?? 0, offsetTop: view?.offsetTop ?? 0 });
  };
  window.visualViewport?.addEventListener('resize', recordViewport, { passive: true });
  window.visualViewport?.addEventListener('scroll', recordViewport, { passive: true });
  window.addEventListener('resize', recordViewport, { passive: true });
  recordViewport();
  let previousBeat = performance.now();
  let lastFrame = performance.now();
  const heartbeat = () => {
    const now = performance.now();
    if (document.visibilityState === 'visible' && now - previousBeat > 650) record('main-thread.timer-gap', { durationMs: Math.round(now - previousBeat) });
    health.timer(now - previousBeat);
    previousBeat = now;
    const report = health.report(now, document.visibilityState === 'visible');
    if (report) record('page.health', report);
    if (moves || wheels || touchMoves || gestureChanges) record('input.counts', { moves, wheels, touchMoves, gestureChanges });
    moves = wheels = touchMoves = gestureChanges = 0;
    send({ type: 'heartbeat', visible: document.visibilityState === 'visible', events: events.splice(0), dropped });
    dropped = 0;
  };
  heartbeat();
  setInterval(heartbeat, 250);
  document.addEventListener('visibilitychange', () => {
    previousBeat = lastFrame = performance.now();
    record('page.visibility', { state: document.visibilityState });
    heartbeat();
  });
  window.addEventListener('pagehide', heartbeat);
  const frame = (now: number) => {
    health.frame(now - lastFrame);
    if (document.visibilityState === 'visible' && now - lastFrame > 200) record('main-thread.frame-gap', { durationMs: Math.round(now - lastFrame) });
    lastFrame = now;
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  // Feature detection keeps this optional on WebKit versions without long-task entries.
  if (typeof PerformanceObserver !== 'undefined' && PerformanceObserver.supportedEntryTypes?.includes('longtask')) {
    new PerformanceObserver(list => { for (const entry of list.getEntries()) record('main-thread.longtask', { durationMs: Math.round(entry.duration), startTime: entry.startTime }); }).observe({ entryTypes: ['longtask'] });
  }
}
