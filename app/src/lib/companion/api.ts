import { parseAck, parseChange, type ShareAck, type ShareChange, type ShareOp, type ShareSnapshot } from "./protocol";

/** HTTP side of the share protocol. The secret travels only in the
 * Authorization header; no URL, error message or log line carries it. */

/** The authority's refusal codes, plus the transport's own. */
export type ShareErrorCode =
  | "conflict-shape" | "conflict-order" | "conflict-background" | "invalid" | "stale-op"
  | "session-ended" | "too-large" | "external-edit" | "io"
  | "gone" | "unauthorized" | "not-found" | "server" | "bad-response";

/** The server answered and refused. A 5xx (`server`) or a malformed answer may not have been applied. */
export class ShareHttpError extends Error {
  constructor(readonly status: number, readonly code: ShareErrorCode, message: string) { super(message); this.name = "ShareHttpError"; }
}
/** The request never got an answer: offline, refused, aborted or timed out. Its fate is unknown. */
export class ShareNetworkError extends Error {
  constructor(message = "Can't reach the Mac.") { super(message); this.name = "ShareNetworkError"; }
}

/** Snapshots include embedded images; the authority refuses anything over 256 MiB serialized. */
export const MAX_SNAPSHOT_BYTES = 260 * 1024 * 1024;

const STATUS_CODE: Record<number, ShareErrorCode> = { 401: "unauthorized", 403: "unauthorized", 404: "not-found", 410: "gone", 413: "too-large", 423: "external-edit" };
const KNOWN = new Set<string>(["conflict-shape", "conflict-order", "conflict-background", "invalid", "stale-op", "session-ended", "too-large", "external-edit", "io", "gone"]);
const FALLBACK: Partial<Record<ShareErrorCode, string>> = {
  unauthorized: "This share link is no longer valid.",
  "not-found": "This canvas is no longer shared.",
  gone: "This canvas is no longer shared.",
  "session-ended": "This canvas is no longer shared.",
  "external-edit": "The canvas changed on the Mac outside Oculus. Saving resumes once it is resolved there.",
  "too-large": "This change is too large to share.",
  server: "The Mac could not save this change.",
};

export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export class ShareApi {
  private readonly base: string;
  private readonly authorization: string;
  constructor(origin: string, shareId: string, secret: string, private readonly fetchImpl: FetchLike) {
    this.base = `${origin}/api/s/${shareId}`;
    this.authorization = `Bearer ${shareId}.${secret}`;
  }

  private async request(path: string, init: RequestInit = {}): Promise<Response> {
    let response: Response;
    try {
      response = await this.fetchImpl(`${this.base}${path}`, {
        ...init,
        headers: { ...(init.headers as Record<string, string> | undefined), Authorization: this.authorization },
        cache: "no-store",
        credentials: "omit",
        referrerPolicy: "no-referrer",
      });
    } catch (error) {
      throw new ShareNetworkError((error as { name?: string })?.name === "AbortError" ? "The Mac took too long to answer." : undefined);
    }
    if (response.ok) return response;
    let body: { code?: unknown; message?: unknown } | null = null;
    try { body = await response.json(); } catch { /* not JSON */ }
    const named = typeof body?.code === "string" && KNOWN.has(body.code) ? body.code as ShareErrorCode : undefined;
    const code = named ?? STATUS_CODE[response.status] ?? (response.status >= 500 ? "server" : "bad-response");
    const message = typeof body?.message === "string" && body.message ? body.message : FALLBACK[code] ?? `The Mac answered ${response.status}.`;
    throw new ShareHttpError(response.status, code, message);
  }

  /** The canvas file text, read as a stream so its size is bounded before it is held. */
  async snapshot(signal?: AbortSignal): Promise<ShareSnapshot> {
    const response = await this.request("/snapshot", { signal });
    const epoch = response.headers.get("X-Oculus-Epoch");
    const revision = Number(response.headers.get("X-Oculus-Revision"));
    if (!epoch || !Number.isSafeInteger(revision) || revision < 0) throw new ShareHttpError(response.status, "bad-response", "The Mac sent an incomplete canvas.");
    const declared = response.headers.get("Content-Length");
    const length = declared === null ? null : Number(declared);
    if (length !== null && length > MAX_SNAPSHOT_BYTES) throw new ShareHttpError(response.status, "too-large", "This canvas is too large to open here.");
    let text: string;
    try { text = await readText(response, length); }
    catch (error) {
      if (error instanceof ShareHttpError) throw error;
      throw new ShareNetworkError("The canvas stopped downloading.");
    }
    return { epoch, revision, text };
  }

  async commit(op: ShareOp, signal?: AbortSignal): Promise<ShareAck> {
    const response = await this.request("/commit", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ clientId: op.clientId, seq: op.seq, opId: op.opId, epoch: op.epoch, transaction: op.transaction }),
      signal,
    });
    let value: unknown;
    try { value = await response.json(); } catch { throw new ShareNetworkError("The Mac's answer was cut off."); }
    const ack = parseAck(value);
    if (!ack) throw new ShareHttpError(response.status, "bad-response", "The Mac sent an unreadable answer.");
    return ack;
  }

  /** Changes after `from`, in revision order; `gone` when the log no longer reaches back that far. */
  async changes(from: number, signal?: AbortSignal): Promise<ShareChange[]> {
    const response = await this.request(`/changes?from=${from}`, { signal });
    let value: unknown;
    try { value = await response.json(); } catch { throw new ShareNetworkError("The Mac's answer was cut off."); }
    if (!Array.isArray(value)) throw new ShareHttpError(response.status, "bad-response", "The Mac sent an unreadable answer.");
    const changes = value.map(parseChange);
    if (changes.some((change) => !change)) throw new ShareHttpError(response.status, "bad-response", "The Mac sent an unreadable answer.");
    return (changes as ShareChange[]).sort((a, b) => a.revision - b.revision);
  }
}

async function readText(response: Response, length: number | null): Promise<string> {
  const reader = response.body?.getReader();
  if (!reader) return response.text();
  const decoder = new TextDecoder();
  const parts: string[] = [];
  let received = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    received += value.byteLength;
    if (received > MAX_SNAPSHOT_BYTES) {
      void reader.cancel().catch(() => {});
      throw new ShareHttpError(response.status, "too-large", "This canvas is too large to open here.");
    }
    parts.push(decoder.decode(value, { stream: true }));
  }
  parts.push(decoder.decode());
  if (length !== null && received !== length) throw new ShareNetworkError("The canvas stopped downloading.");
  return parts.join("");
}
