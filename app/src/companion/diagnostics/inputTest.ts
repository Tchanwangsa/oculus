import { startCompanionDiagnostics } from './index';
import { secureId } from '@/lib/canvas/ids';
import { traceCanvas } from '@/lib/canvas/diagnostics';

/** A separate entry keeps the comparison pad free of React, sharing and notebook work. */
async function start(): Promise<void> {
  // This is only a trace identity, never a credential used for a sharing request.
  const recording = startCompanionDiagnostics({ shareId: secureId().replace(/-/g, ''), secret: '' }, 'input-test');
  const canvas = document.querySelector('canvas')!;
  const counts = document.querySelector('#counts')!;
  const test = document.querySelector<HTMLButtonElement>('#test')!;
  const context = canvas.getContext('2d');
  const ink = getComputedStyle(canvas).color;
  const contacts = new Map<number, { x: number; y: number }>();
  let pointers = 0, touches = 0, taps = 0;
  const showCounts = () => { counts.textContent = `Pointer contacts: ${pointers} · Touch contacts: ${touches}`; };
  const resize = () => {
    const rect = canvas.getBoundingClientRect(), ratio = Math.min(devicePixelRatio || 1, 2);
    const width = Math.round(rect.width * ratio), height = Math.round(rect.height * ratio);
    if (canvas.width === width && canvas.height === height) return;
    canvas.width = width; canvas.height = height;
    context?.setTransform(ratio, 0, 0, ratio, 0, 0);
    contacts.clear();
  };
  resize();
  window.addEventListener('resize', resize);
  const position = (event: PointerEvent) => {
    const rect = canvas.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  };
  canvas.addEventListener('pointerdown', event => {
    event.preventDefault();
    pointers++; showCounts();
    const point = position(event);
    if (contacts.size < 10) contacts.set(event.pointerId, point);
    if (context) {
      context.fillStyle = ink;
      context.beginPath(); context.arc(point.x, point.y, 2, 0, Math.PI * 2); context.fill();
    }
  });
  canvas.addEventListener('pointermove', event => {
    const previous = contacts.get(event.pointerId);
    if (!previous || !context) return;
    const point = position(event);
    context.strokeStyle = ink; context.lineWidth = 3; context.lineCap = 'round';
    context.beginPath(); context.moveTo(previous.x, previous.y); context.lineTo(point.x, point.y); context.stroke();
    contacts.set(event.pointerId, point);
  });
  for (const type of ['pointerup', 'pointercancel', 'pointerleave']) canvas.addEventListener(type, event => { contacts.delete((event as PointerEvent).pointerId); });
  for (const type of ['touchstart', 'touchmove', 'touchend']) document.body.addEventListener(type, event => {
    if (event.target !== canvas) return;
    event.preventDefault();
    if (type === 'touchstart') { touches++; showCounts(); }
  }, { capture: true, passive: false });
  test.addEventListener('click', () => {
    test.textContent = `Test button · ${++taps} taps`;
    traceCanvas('probe.tap', { taps });
  });
  document.querySelector('#clear')!.addEventListener('click', () => { context?.clearRect(0, 0, canvas.width, canvas.height); contacts.clear(); traceCanvas('probe.clear'); });
  document.querySelector('#logging')!.textContent = await recording ? 'Recording input on the Mac' : 'Local logging is unavailable';
  traceCanvas('probe.started');
}
void start();
