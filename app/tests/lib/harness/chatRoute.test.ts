import { expect, test } from "bun:test";
import { chatHref, chatScope, chatThreadId } from "@/lib/harness/route";

test("a new chat's route carries an entry point's scope, a thread's never does", () => {
  expect(chatHref(null)).toBe("/chat");
  expect(chatHref(null, null, { subject: "general" })).toBe("/chat?s=general");
  expect(chatHref(null, null, { subject: 12 })).toBe("/chat?s=12");
  expect(chatHref(3, "Week 2", { subject: 12 })).toBe("/chat?t=3&n=Week%202");
});

test("chatScope reads General or a subject id off the route, and nothing else", () => {
  expect(chatScope("?s=general")).toBe("general");
  expect(chatScope("?s=12")).toBe(12);
  expect(chatScope("?t=4&s=12")).toBe(12);
  for (const search of ["", "?s=", "?s=General", "?s=0", "?s=-2", "?s=1.5", "?s=12x", "?subject=12"]) {
    expect(chatScope(search)).toBeNull();
  }
});

test("the scope and the thread id are read independently", () => {
  expect(chatThreadId(chatHref(null, null, { subject: 12 }).slice(5))).toBeNull();
  expect(chatScope(chatHref(5).slice(5))).toBeNull();
});
