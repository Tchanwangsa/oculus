import { expect, test } from "bun:test";
import { planDrop, type DraggedEntry } from "@/lib/drive/drop";

const own = (rel: string): DraggedEntry => ({ rel, zone: "own" });
const synced = (rel: string): DraggedEntry => ({ rel, zone: "synced" });

test("a folder that is not the student's refuses every drop", () => {
  expect(planDrop([own("s/A/a.pdf")], { rel: "s/Course", writable: false })).toEqual({ kind: "refuse" });
});

test("own entries move, synced ones copy", () => {
  expect(planDrop([own("s/A/a.pdf"), synced("s/Course/Files/w3.pdf")], { rel: "s/B", writable: true })).toEqual({
    kind: "go",
    move: ["s/A/a.pdf"],
    copy: ["s/Course/Files/w3.pdf"],
  });
});

test("dropping where an entry already is moves nothing", () => {
  expect(planDrop([own("s/A/a.pdf")], { rel: "s/A", writable: true })).toEqual({ kind: "none" });
  expect(planDrop([own("s/A/a.pdf"), own("s/b.pdf")], { rel: "s/A", writable: true })).toEqual({
    kind: "go",
    move: ["s/b.pdf"],
    copy: [],
  });
});

test("a folder never drops into itself or below itself", () => {
  expect(planDrop([own("s/A")], { rel: "s/A", writable: true })).toEqual({ kind: "none" });
  expect(planDrop([own("s/A")], { rel: "s/A/B", writable: true })).toEqual({ kind: "none" });
  expect(planDrop([own("s/A")], { rel: "s/AB", writable: true }).kind).toBe("go");
});
