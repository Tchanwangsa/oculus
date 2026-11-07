import { mkdir, readFile, writeFile, stat, rename, appendFile, unlink } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { randomBytes, timingSafeEqual } from 'node:crypto';

export const MAX_BODY = 64 * 1024;
const MAX_LOG = 10 * 1024 * 1024;
export function allowedOrigin(origin: string | null, host: string | null): string | null {
  try {
    const url = new URL(origin ?? '');
    const destination = new URL(`http://${host}`);
    return url.protocol === 'http:' && url.hostname === destination.hostname && Number(url.port) >= 43821 && Number(url.port) <= 43840 ? url.origin : null;
  } catch { return null; }
}
export async function readBatch(request: Request): Promise<unknown> {
  if (Number(request.headers.get('content-length')) > MAX_BODY) throw new Error('too-large');
  const reader = request.body?.getReader();
  if (!reader) throw new Error('empty');
  const chunks: Uint8Array[] = [];
  let length = 0;
  for (;;) {
    const part = await reader.read();
    if (part.done) break;
    length += part.value.byteLength;
    if (length > MAX_BODY) { await reader.cancel(); throw new Error('too-large'); }
    chunks.push(part.value);
  }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  return JSON.parse(new TextDecoder().decode(bytes));
}
export function logLines(value: unknown, receivedAt = Date.now()): string {
  const batch = value as { session?: unknown; shareId?: unknown; dropped?: unknown; events?: unknown };
  if (!batch || typeof batch.session !== 'string' || !/^[a-zA-Z0-9-]{1,80}$/.test(batch.session) || typeof batch.shareId !== 'string' || !/^[a-zA-Z0-9-]{1,80}$/.test(batch.shareId) || !Array.isArray(batch.events) || batch.events.length > 100) throw new Error('invalid');
  return batch.events.map(event => {
    if (!event || typeof event.name !== 'string' || !/^[a-zA-Z0-9_.-]{1,80}$/.test(event.name) || typeof event.at !== 'number' || !Number.isFinite(event.at) || !event.fields || typeof event.fields !== 'object' || Array.isArray(event.fields)) throw new Error('invalid');
    const fields: Record<string, string | number | boolean | null> = {};
    for (const [key, item] of Object.entries(event.fields).slice(0, 32)) {
      if (!/^[a-zA-Z0-9_]{1,64}$/.test(key)) continue;
      if (typeof item === 'string') fields[key] = item.slice(0, 3000);
      else if (item === null || typeof item === 'boolean' || typeof item === 'number' && Number.isFinite(item)) fields[key] = item;
    }
    return JSON.stringify({ receivedAt, at: event.at, session: batch.session, shareId: batch.shareId, dropped: Number(batch.dropped) || 0, name: event.name, fields }) + '\n';
  }).join('');
}

if (import.meta.main) {
  const root = resolve(process.argv[2] ?? '..');
  const log = resolve(root, 'data/logs/companion-input.jsonl');
  const configPath = resolve(root, 'app/dist/companion/assets/diagnostics-config.json');
  const token = randomBytes(32).toString('hex');
  let writes = Promise.resolve();
  let pending = 0;
  const server = Bun.serve({
    hostname: '::', port: Number(process.env.OCULUS_DIAGNOSTICS_PORT ?? 43841), maxRequestBodySize: MAX_BODY, idleTimeout: 5,
    async fetch(request) {
      const origin = allowedOrigin(request.headers.get('origin'), request.headers.get('host'));
      const headers = { 'Access-Control-Allow-Origin': origin ?? 'null', 'Access-Control-Allow-Methods': 'POST, OPTIONS', 'Access-Control-Allow-Headers': 'Authorization, Content-Type', 'Access-Control-Allow-Private-Network': 'true', 'Cache-Control': 'no-store', Vary: 'Origin' };
      if (new URL(request.url).pathname === '/health' && request.method === 'GET') return Response.json({ ready: true }, { headers });
      if (!origin) return new Response(null, { status: 403, headers });
      if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers });
      const auth = Buffer.from(request.headers.get('authorization') ?? '');
      const expected = Buffer.from(`Bearer ${token}`);
      if (auth.length !== expected.length || !timingSafeEqual(auth, expected)) return new Response(null, { status: 401, headers });
      if (new URL(request.url).pathname !== '/events' || request.method !== 'POST') return new Response(null, { status: 404, headers });
      if (pending >= 8) return new Response(null, { status: 429, headers });
      pending++;
      try {
        const lines = logLines(await readBatch(request));
        const write = writes.then(async () => {
          await mkdir(dirname(log), { recursive: true });
          if (await stat(log).then(value => value.size >= MAX_LOG, () => false)) {
            for (let i = 2; i >= 0; i--) await rename(i ? `${log}.${i}` : log, `${log}.${i + 1}`).catch(() => {});
          }
          await appendFile(log, lines);
        });
        writes = write.catch(() => {});
        await write;
        return Response.json({ serverTime: Date.now() }, { headers });
      } catch { return new Response(null, { status: 400, headers }); }
      finally { pending--; }
    },
  });
  const config = JSON.stringify({ port: server.port, token });
  const publishConfig = async () => {
    if (await readFile(configPath, 'utf8').catch(() => '') === config) return;
    await mkdir(dirname(configPath), { recursive: true });
    await writeFile(configPath, config);
  };
  await publishConfig();
  const timer = setInterval(() => { void publishConfig().catch(() => {}); }, 2000);
  const stop = async () => {
    clearInterval(timer);
    await server.stop(true);
    await writes;
    if (await readFile(configPath, 'utf8').catch(() => '') === config) await unlink(configPath).catch(() => {});
    process.exit(0);
  };
  process.once('SIGINT', () => { void stop(); });
  process.once('SIGTERM', () => { void stop(); });
  console.log(`Companion diagnostics receiver listening on port ${server.port}; log: ${log}`);
}
