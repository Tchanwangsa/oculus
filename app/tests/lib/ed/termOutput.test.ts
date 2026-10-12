import { describe, expect, test } from "bun:test";

import { createTermSink, decodeBase64, runExitLines, type TermView } from "@/lib/ed/termOutput";

function fakeView() {
  const written: string[] = [];
  const view: TermView = {
    write: (d) => written.push(typeof d === "string" ? d : new TextDecoder().decode(d)),
    reset: () => written.push("<reset>"),
    size: () => ({ cols: 100, rows: 30 }),
    focus: () => {},
  };
  return { view, written };
}

describe("term output", () => {
  test("base64 decodes to the raw bytes", () => {
    const bytes = decodeBase64(btoa("hi\r\n\xff"));
    expect([...bytes]).toEqual([104, 105, 13, 10, 255]);
  });

  test("output before the view mounts is replayed on attach", () => {
    const sink = createTermSink();
    expect(sink.size()).toBeNull();
    sink.write("a");
    sink.write(new TextEncoder().encode("b"));
    const { view, written } = fakeView();
    sink.attach(view);
    sink.write("c");
    expect(written).toEqual(["a", "b", "c"]);
    expect(sink.size()).toEqual({ cols: 100, rows: 30 });
  });

  test("a note starts its own line and is dim", () => {
    const sink = createTermSink();
    const { view, written } = fakeView();
    sink.attach(view);
    sink.write("partial");
    sink.note("Program exited with code 0");
    sink.note("again");
    expect(written.slice(1)).toEqual([
      "\r\n\x1b[2mProgram exited with code 0\x1b[22m\r\n",
      "\x1b[2magain\x1b[22m\r\n",
    ]);
  });

  test("reset drops the queue and clears the view", () => {
    const sink = createTermSink();
    sink.write("lost");
    sink.reset();
    const { view, written } = fakeView();
    sink.attach(view);
    sink.reset();
    expect(written).toEqual(["<reset>"]);
  });

  test("a run ends with Ed's message, then how it exited", () => {
    expect(runExitLines({ code: 0, signal: null, message: null })).toEqual(["Program exited with code 0"]);
    expect(runExitLines({ code: null, signal: "SIGKILL", message: null })).toEqual(["Program stopped by signal SIGKILL"]);
    expect(runExitLines({ code: null, signal: null, message: "No resources available" })).toEqual([
      "No resources available",
    ]);
    expect(runExitLines({ code: 1, signal: null, message: "build failed\n" })).toEqual([
      "build failed",
      "Program exited with code 1",
    ]);
  });
});
