import { expect, test } from "bun:test";
import { liveScope, parseChatScope, scopeSubject } from "@/lib/harness/scope";

test("a scope parses from General or a positive whole id, nothing else", () => {
  expect(parseChatScope("general")).toBe("general");
  expect(parseChatScope("12")).toBe(12);
  for (const raw of [null, undefined, "", "General", "0", "-3", "1.5", "12a", " 12", "99999999999999999999"]) {
    expect(parseChatScope(raw)).toBeNull();
  }
});

test("General files under no subject; a subject under itself", () => {
  expect(scopeSubject("general")).toBeNull();
  expect(scopeSubject(4)).toBe(4);
});

test("only a subject still listed stays chosen", () => {
  const subjects = [{ id: 4 }];
  expect(liveScope("general", [])).toBe("general");
  expect(liveScope(4, subjects)).toBe(4);
  expect(liveScope(5, subjects)).toBeNull();
  expect(liveScope(undefined, subjects)).toBeNull();
});
