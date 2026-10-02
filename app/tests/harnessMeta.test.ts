import { expect, test } from "bun:test";
import {
  messageAt, parseErrorMeta, parseItemMeta, parsePermissionMeta, parseToolMeta,
  type HarnessItem,
} from "../src/lib/harness";

const row = (meta: string | null): HarnessItem => ({
  id: 1, thread_id: 1, kind: "tool", ref_id: "call", content: "tool", meta,
  created_at: "2026-01-01 00:00:00",
});

test("malformed metadata never exposes an approval or crashes a tool update", () => {
  for (const meta of [null, "", "broken", "null", "[]", '"text"', "42", "false"]) {
    const item = row(meta);
    expect(parseToolMeta(item)).toEqual({});
    expect(messageAt(item)).toBeNull();
    expect(parseErrorMeta(item)).toEqual({});
    expect(parsePermissionMeta(item).rule).toBeNull();
  }
});

test("metadata caches follow immutable row replacement and preserve all tool fields", () => {
  const item = row('{"kind":"bash","input":{"args":[]},"ok":false,"at":0}');
  const meta = parseItemMeta(item);
  expect(parseItemMeta(item)).toBe(meta);
  expect(parseToolMeta(item)).toEqual(meta);
  expect(parseToolMeta(item)).toBe(parseToolMeta(item));
  expect(messageAt(item)).toBe(0);
  const finished = { ...item, meta: JSON.stringify({ ...meta, ok: true, output: "done" }) };
  expect(parseToolMeta(finished)).toEqual({ kind: "bash", input: { args: [] }, ok: true, at: 0, output: "done" });
  expect(parseToolMeta(item).ok).toBe(false);
});

test("only recognized auth providers and non-empty permission strings survive", () => {
  expect(parseErrorMeta(row('{"auth":"codex"}'))).toEqual({ auth: "codex" });
  expect(parseErrorMeta(row('{"auth":"invented"}'))).toEqual({});
  expect(messageAt(row('{"at":"3"}'))).toBeNull();
  expect(parsePermissionMeta(row('{"tool":"bash","action":false,"rule":"","target":0}')))
    .toEqual({ tool: "bash", action: undefined, rule: null, target: null });
});
