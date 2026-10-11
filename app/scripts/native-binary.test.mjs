import { afterEach, beforeEach, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fetchNative } from "./native-binary.mjs";
import { hostTriple } from "./runtime.mjs";

const host = hostTriple();
const originalFetch = globalThis.fetch;
const originalArgs = [...process.argv];
let scratch;
let dest;
let downloads;

beforeEach(() => {
  scratch = mkdtempSync(join(tmpdir(), "oculus-download-test-"));
  dest = join(scratch, "binary");
  downloads = 0;
  process.argv.push("--force");
  globalThis.fetch = async () => {
    downloads++;
    return new Response(Buffer.alloc(1_000_001, "n"));
  };
});
afterEach(() => {
  globalThis.fetch = originalFetch;
  process.argv.splice(0, process.argv.length, ...originalArgs);
  rmSync(scratch, { recursive: true, force: true });
});

function download(install = (buf, partial) => writeFileSync(partial, buf), sha256) {
  return fetchNative("test", { [host]: "test.bin" }, (asset) => ({
    asset, dest, url: "https://invalid.test/binary", install, sha256,
  }));
}

const sha256Of = (buf) => createHash("sha256").update(buf).digest("hex");

test("downloads follow redirects and install the complete payload", async () => {
  const requests = [];
  globalThis.fetch = async (...args) => {
    requests.push(args);
    return new Response(Buffer.alloc(1_000_001, "n"));
  };
  await download();
  expect(requests).toEqual([["https://invalid.test/binary", { redirect: "follow" }]]);
  expect(readFileSync(dest).length).toBe(1_000_001);
});

test("a cached binary skips downloading unless forced", async () => {
  process.argv.pop();
  writeFileSync(dest, Buffer.alloc(1_000_001, "o"));
  await download();
  expect(downloads).toBe(0);
  expect(readFileSync(dest)[0]).toBe("o".charCodeAt(0));
});

test("a short download never replaces the previous binary", async () => {
  writeFileSync(dest, "previous");
  globalThis.fetch = async () => new Response("truncated");
  await expect(download()).rejects.toThrow("suspiciously small download");
  expect(readFileSync(dest, "utf8")).toBe("previous");
  expect(readdirSync(scratch)).toEqual(["binary"]);
});

test("an HTTP failure leaves the cached binary intact", async () => {
  writeFileSync(dest, "previous");
  globalThis.fetch = async () => new Response("no", { status: 503 });
  await expect(download()).rejects.toThrow("503");
  expect(readFileSync(dest, "utf8")).toBe("previous");
});

test("failed installation cleans its partial file and preserves the cache", async () => {
  writeFileSync(dest, "previous");
  await expect(download((buf, partial) => {
    writeFileSync(partial, buf);
    throw new Error("cannot unpack");
  })).rejects.toThrow("cannot unpack");
  expect(readFileSync(dest, "utf8")).toBe("previous");
  expect(readdirSync(scratch)).toEqual(["binary"]);
});

test("failed rename preserves the destination and removes the partial", async () => {
  mkdirSync(dest);
  writeFileSync(join(dest, "previous"), "keep");
  await expect(download()).rejects.toThrow();
  expect(readFileSync(join(dest, "previous"), "utf8")).toBe("keep");
  expect(readdirSync(scratch)).toEqual(["binary"]);
});

test("concurrent downloads use independent partial files", async () => {
  const partials = [];
  let release;
  const bothInstalling = new Promise((resolve) => { release = resolve; });
  await Promise.all([download(install), download(install)]);
  expect(new Set(partials).size).toBe(2);
  expect(existsSync(dest)).toBe(true);
  expect(readdirSync(scratch)).toEqual(["binary"]);

  async function install(buf, partial) {
    partials.push(partial);
    writeFileSync(partial, buf);
    if (partials.length === 2) release();
    await bothInstalling;
  }
});

test("a download that misses its pinned SHA-256 never replaces the cache", async () => {
  writeFileSync(dest, "previous");
  await expect(download(undefined, "0".repeat(64))).rejects.toThrow("expected 0000");
  expect(readFileSync(dest, "utf8")).toBe("previous");
  expect(readdirSync(scratch)).toEqual(["binary"]);
});

test("a cached binary with another hash is downloaded again", async () => {
  process.argv.pop();
  writeFileSync(dest, Buffer.alloc(1_000_001, "o"));
  await download(undefined, sha256Of(Buffer.alloc(1_000_001, "n")));
  expect(downloads).toBe(1);
  expect(readFileSync(dest)[0]).toBe("n".charCodeAt(0));
});

test("a cached binary matching its pinned hash is kept", async () => {
  process.argv.pop();
  writeFileSync(dest, Buffer.alloc(1_000_001, "o"));
  await download(undefined, sha256Of(Buffer.alloc(1_000_001, "o")));
  expect(downloads).toBe(0);
});
