import { installCanvasTrace, type TraceFields } from '@/lib/canvas/diagnostics';

type Line = { at: number; text: string; alert: boolean };

const clock = (at = Date.now()) => new Date(at).toTimeString().slice(0, 8) + '.' + String(at % 1000).padStart(3, '0');

/** One line per trace worth seeing on a recording; null for the rest. `alert` lines turn the panel red for a moment. */
function describe(name: string, fields: TraceFields): Omit<Line, 'at'> | null {
  switch (name) {
    case 'gesture.up': return fields.gesture === 'none' ? null : { alert: false, text: `up → ${fields.gesture}  (${fields.tool})` };
    case 'gesture.cancel': return { alert: true, text: `cancelled ${fields.gesture}  (${fields.tool})` };
    case 'gesture.drop': return { alert: false, text: `second finger: ${fields.gesture} dropped, now navigating` };
    case 'input.ignored': return { alert: false, text: `ignored (${fields.reason})` };
    case 'tool.committed': return { alert: false, text: `tool ${fields.tool}` };
    case 'camera.pan': return fields.verdict === 'glide'
      ? { alert: false, text: `GLIDE ${Math.hypot(Number(fields.vx), Number(fields.vy)).toFixed(1)} px/ms  ${fields.source}  travel ${fields.travel}  span ${fields.spanMs}ms` }
      : { alert: false, text: `pan end (${fields.verdict})  ${fields.source}  travel ${fields.travel}` };
    case 'camera.glide-end': return { alert: false, text: `glide end ${fields.reason}  moved ${fields.dx},${fields.dy} in ${fields.durationMs}ms` };
    case 'camera.jump': return fields.cause === 'two-finger' ? null
      : { alert: true, text: `JUMP ${fields.dx},${fields.dy}  ${fields.cause}${fields.clamped ? ' (clamped)' : ''}  zoom ${fields.fromZoom}→${fields.toZoom}` };
    case 'canvas.resize': return { alert: false, text: `resize ${fields.fromWidth}×${fields.fromHeight} → ${fields.width}×${fields.height}` };
    default: return null;
  }
}

/**
 * The companion's debug view, for screen recordings: a ring under each finger (red when the system cancels it), a filled
 * dot under the Pencil and a hollow one while it hovers, and a panel with a clock to match the video to the diagnostics
 * log and the last few gesture and camera decisions. It never takes input. Returns its removal.
 */
export function showGestureOverlay(): () => void {
  const root = document.createElement('div');
  root.setAttribute('aria-hidden', 'true');
  root.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:2147483647;overflow:hidden';
  const panel = document.createElement('div');
  panel.style.cssText = 'position:absolute;left:8px;bottom:8px;max-width:calc(100% - 16px);font:600 11px/1.35 system-ui,sans-serif;font-variant-numeric:tabular-nums;color:#fff;background:rgba(0,0,0,.6);padding:3px 7px;border-radius:7px;white-space:pre';
  const hover = document.createElement('div');
  hover.style.cssText = 'position:absolute;left:0;top:0;width:14px;height:14px;border-radius:50%;border:2px solid rgba(0,0,0,.7);box-sizing:border-box;display:none';
  root.append(hover, panel);
  document.body.append(root);

  const lines: Line[] = [];
  const render = () => {
    const now = Date.now();
    while (lines.length && now - lines[0].at > 8000) lines.shift();
    panel.textContent = [clock(now), ...lines.map(line => `${clock(line.at)}  ${line.text}`)].join('\n');
    panel.style.background = lines.some(line => line.alert && now - line.at < 1500) ? 'rgba(200,30,30,.8)' : 'rgba(0,0,0,.6)';
  };
  const timer = setInterval(render, 100);
  render();

  const dots = new Map<number, HTMLDivElement>();
  let hoverSeen = 0;
  const place = (dot: HTMLDivElement, event: PointerEvent) => {
    const size = event.pointerType === 'pen' ? 8 : Math.max(18, Math.min(160, Math.max(event.width || 0, event.height || 0)));
    dot.style.width = dot.style.height = `${size}px`;
    dot.style.transform = `translate(${event.clientX - size / 2}px, ${event.clientY - size / 2}px)`;
  };
  const down = (event: PointerEvent) => {
    if (event.pointerType === 'mouse') return;
    dots.get(event.pointerId)?.remove();
    const dot = document.createElement('div');
    dot.style.cssText = event.pointerType === 'pen'
      ? 'position:absolute;left:0;top:0;border-radius:50%;background:rgba(0,0,0,.75)'
      : 'position:absolute;left:0;top:0;border-radius:50%;border:2px solid rgba(94,106,210,.9);background:rgba(94,106,210,.18);box-sizing:border-box';
    place(dot, event);
    root.append(dot);
    dots.set(event.pointerId, dot);
    if (event.pointerType === 'pen') hover.style.display = 'none';
  };
  const move = (event: PointerEvent) => {
    const dot = dots.get(event.pointerId);
    if (dot) { place(dot, event); return; }
    // A Pencil moving with no contact is hovering.
    if (event.pointerType === 'pen' && !event.buttons) {
      hoverSeen = performance.now();
      hover.style.display = 'block';
      hover.style.transform = `translate(${event.clientX - 7}px, ${event.clientY - 7}px)`;
    }
  };
  const end = (event: PointerEvent) => {
    const dot = dots.get(event.pointerId);
    if (!dot) return;
    dots.delete(event.pointerId);
    if (event.type === 'pointercancel') { dot.style.borderColor = 'rgba(220,40,40,.95)'; dot.style.background = 'rgba(220,40,40,.25)'; }
    dot.style.transition = 'opacity 600ms ease';
    requestAnimationFrame(() => { dot.style.opacity = '0'; });
    setTimeout(() => dot.remove(), 700);
  };
  const leave = (event: PointerEvent) => { if (event.pointerType === 'pen') hover.style.display = 'none'; };
  const hoverTimer = setInterval(() => { if (hover.style.display === 'block' && performance.now() - hoverSeen > 300) hover.style.display = 'none'; }, 150);
  const options = { capture: true, passive: true } as const;
  const listeners: [string, (event: PointerEvent) => void][] = [['pointerdown', down], ['pointermove', move], ['pointerup', end], ['pointercancel', end], ['pointerleave', leave]];
  for (const [type, listener] of listeners) window.addEventListener(type, listener as EventListener, options);

  const uninstall = installCanvasTrace({
    event(name, fields) {
      const line = describe(name, fields);
      if (!line) return;
      lines.push({ at: Date.now(), ...line });
      if (lines.length > 7) lines.shift();
      render();
    },
  });

  return () => {
    uninstall();
    clearInterval(timer); clearInterval(hoverTimer);
    for (const [type, listener] of listeners) window.removeEventListener(type, listener as EventListener, options);
    root.remove();
  };
}
