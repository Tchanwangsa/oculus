import { expect, test } from "bun:test";
import { splitAttachments, withAttachments } from "@/lib/harness/attachments";

test("a rewound message gives its pictures back and keeps its words", () => {
  const message = withAttachments("test test", ["./attachments/a.png", "./attachments/b.jpg"]);
  expect(splitAttachments(message)).toEqual({
    text: "test test",
    paths: ["./attachments/a.png", "./attachments/b.jpg"],
  });
});

test("a message of only pictures has no words left", () => {
  expect(splitAttachments(withAttachments("", ["./attachments/a.png"]))).toEqual({
    text: "",
    paths: ["./attachments/a.png"],
  });
});

test("a last paragraph of anything else is text", () => {
  for (const message of [
    "see `./attachments/a.png` here",
    "words\n\n`courses/MAST10006/files/a.pdf`",
    "words\n\n`./attachments/a.png` and more",
    "no pictures",
  ]) {
    expect(splitAttachments(message)).toEqual({ text: message, paths: [] });
  }
});
