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
    "words\n\n`courses/MAST10006/Course/Files/a.pdf`",
    "words\n\n`./attachments/a.png` and more",
    // A PDF or text file is a chip in the sentence, never a lifted line.
    "words\n\n`./attachments/a.png` `./attachments/b.pdf`",
    "no pictures",
  ]) {
    expect(splitAttachments(message)).toEqual({ text: message, paths: [] });
  }
});
