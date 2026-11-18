import { describe, expect, test } from "bun:test";
import { prioritiseJob } from "@/lib/pipeline/queueOrder";

const job = (relativePath: string) => ({ relativePath });
const paths = (queue: { relativePath: string }[]) => queue.map((j) => j.relativePath);
const queue = ["a", "b", "c", "d"].map(job);

describe("prioritiseJob", () => {
  test("moves the file to the front and keeps the rest in order", () => {
    const next = prioritiseJob(queue, [], "c");
    expect(paths(next!.queue)).toEqual(["c", "a", "b", "d"]);
    expect(next!.pushed).toEqual(["c"]);
  });

  test("several pushed files keep the order they were pushed in", () => {
    const first = prioritiseJob(queue, [], "d")!;
    const second = prioritiseJob(first.queue, first.pushed, "b")!;
    expect(paths(second.queue)).toEqual(["d", "b", "a", "c"]);
    expect(second.pushed).toEqual(["d", "b"]);
  });

  test("pushing a pushed file again moves it behind the others pushed", () => {
    const first = prioritiseJob(queue, [], "d")!;
    const second = prioritiseJob(first.queue, first.pushed, "b")!;
    const third = prioritiseJob(second.queue, second.pushed, "d")!;
    expect(paths(third.queue)).toEqual(["b", "d", "a", "c"]);
  });

  test("files that left the queue fall out of the pushed list", () => {
    const next = prioritiseJob(queue.slice(1), ["a", "b"], "d")!;
    expect(next.pushed).toEqual(["b", "d"]);
    expect(paths(next.queue)).toEqual(["b", "d", "c"]);
  });

  test("a file that is not waiting changes nothing", () => {
    expect(prioritiseJob(queue, [], "running")).toBeNull();
  });
});
