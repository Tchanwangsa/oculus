import { StallWatch, type DiagnosticEvent, type DiagnosticMessage } from './protocol';

const watch = new StallWatch();
let config: Extract<DiagnosticMessage, { type: 'init' }> | null = null;
let queue: DiagnosticEvent[] = [];
let sending = false;
let dropped = 0;
function append(event: DiagnosticEvent): void {
  queue.push(event);
  if (queue.length > 1000) { queue.shift(); dropped++; }
}
async function flush(): Promise<void> {
  if (!config || sending || !queue.length) return;
  sending = true;
  const events = queue.splice(0, 60);
  const lost = dropped;
  dropped = 0;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 4000);
  try {
    const url = new URL(`http://${config.hostname}/`);
    url.port = String(config.port);
    url.pathname = '/events';
    let body = JSON.stringify({ session: config.session, shareId: config.shareId, events, dropped: lost });
    while (new TextEncoder().encode(body).byteLength > 48 * 1024 && events.length > 1) {
      queue.unshift(...events.splice(Math.ceil(events.length / 2)));
      body = JSON.stringify({ session: config.session, shareId: config.shareId, events, dropped: lost });
    }
    const response = await fetch(url, {
      method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${config.token}` },
      body,
      credentials: 'omit', referrerPolicy: 'no-referrer', signal: controller.signal,
    });
    if (!response.ok) throw new Error('Diagnostics receiver unavailable');
  } catch {
    queue = [...events, ...queue];
    dropped += lost;
    if (queue.length > 1000) { dropped += queue.length - 1000; queue = queue.slice(-1000); }
  } finally { clearTimeout(timeout); sending = false; }
}
self.onmessage = (message: MessageEvent<DiagnosticMessage>) => {
  const value = message.data;
  if (value.type === 'init') { config = value; return; }
  if (value.type === 'begin') {
    if (watch.active.size < 64) watch.active.set(value.id, { name: value.name, at: value.at });
    return;
  }
  if (value.type === 'finish') { watch.active.delete(value.id); return; }
  const recovered = watch.heartbeat(Date.now(), value.visible);
  if (recovered) append(recovered);
  for (const event of value.events) append(event);
  dropped += value.dropped;
};
setInterval(() => {
  const stalled = watch.check(Date.now());
  if (stalled) append(stalled);
  void flush();
}, 1000);
