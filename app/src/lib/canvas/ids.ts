/**
 * RFC 4122 v4 id from the platform CSPRNG. `crypto.randomUUID` exists only in
 * secure contexts, and the companion page is served over plain HTTP on the LAN,
 * so fall back to `getRandomValues` (available everywhere); never to Math.random.
 */
export function secureId(): string {
  const c = globalThis.crypto;
  if (typeof c?.randomUUID === "function") return c.randomUUID();
  if (typeof c?.getRandomValues !== "function") throw new Error("No secure random source is available.");
  const b = c.getRandomValues(new Uint8Array(16));
  b[6] = (b[6] & 0x0f) | 0x40;
  b[8] = (b[8] & 0x3f) | 0x80;
  const hex = Array.from(b, (n) => n.toString(16).padStart(2, "0"));
  return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10).join("")}`;
}
