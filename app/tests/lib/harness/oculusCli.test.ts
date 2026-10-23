import { describe, expect, test } from "bun:test";
import { describeOculusCli, shellCommands, summariseOculusCli } from "@/lib/harness";

const title = (line: string, done = true) => summariseOculusCli(describeOculusCli(line), done);

describe("oculus CLI row titles", () => {
  test("shell lines split at separators, keep quotes and drop pipes and redirects", () => {
    expect(shellCommands(`oculus grep "a|b" -n 25 2>&1 | head -30 && oculus memory list`)).toEqual([
      ["oculus", "grep", "a|b", "-n", "25"],
      ["oculus", "memory", "list"],
    ]);
    expect(shellCommands(`cd 'my dir'; oculus read x.pdf`)).toEqual([["cd", "my dir"], ["oculus", "read", "x.pdf"]]);
  });

  test("memory commands say what happened to which memory", () => {
    expect(title(`oculus memory write misconception-deutsch-ancilla -s MULT20015_2026_SM2 --type reference --about "Deutsch algorithm confusion" --text "…"`))
      .toEqual({ title: "Saved a memory", em: "Deutsch algorithm confusion", icon: "memory" });
    expect(title(`oculus memory write lab5-is-dj-template --text x`, false)?.title).toBe("Saving a memory");
    expect(title(`oculus memory read tensor-factor-working-style -s MULT20015_2026_SM2`))
      .toEqual({ title: "Read memory", em: "tensor-factor-working-style", icon: "memory" });
    expect(title(`oculus memory list -s MULT20015_2026_SM2`)?.em).toBe("MULT20015");
    expect(title(`oculus memory list --all`)?.em).toBe("all subjects");
  });

  test("library searches and reads name the query, file and subject", () => {
    expect(title(`oculus grep "word limit" -n 25 2>&1 | head -30`))
      .toEqual({ title: "Searched the library for", em: `"word limit"`, icon: "search" });
    expect(title(`oculus search "phase kickback" -s MULT20015`, false))
      .toEqual({ title: "Searching the library for", em: `"phase kickback" · MULT20015`, icon: "search" });
    expect(title(`oculus read courses/COMP30026_2026_SM2/files/15.pdf -p 3-5`)?.em).toBe("15.pdf · p. 3-5 · COMP30026");
    expect(title(`/Applications/Oculus.app/Contents/MacOS/oculus read Lecture_8.pdf`)?.em).toBe("Lecture_8.pdf");
  });

  test("a chain of one command is counted; a mixed chain is titled by its first", () => {
    const moves = Array.from({ length: 7 }, (_, i) => `oculus task move ${113 + i} -c todo`).join(" && ");
    expect(title(moves)).toEqual({ title: "Moved 7 tasks to", em: "To do", icon: "task" });
    expect(title(`oculus task move 123 -c done`)).toEqual({ title: "Moved task to", em: "Done", icon: "task" });
    expect(title(`oculus memory read a && oculus grep x`)).toEqual({ title: "Read memory", em: "a · +1 more", icon: "memory" });
  });

  test("planning and unknown commands", () => {
    expect(title(`oculus project create "Assignment 2 — catch-up" -s MULT20015_2026_SM2 --due 2026-10-13`))
      .toEqual({ title: "Created project", em: "Assignment 2 — catch-up", icon: "project" });
    expect(title(`oculus task add --title "Revise L12" -p 4`)?.em).toBe("Revise L12");
    expect(title(`oculus --help`)).toEqual({ title: "Ran oculus", em: undefined, icon: "library" });
    expect(title(`ls -la`)).toBeNull();
  });

  test("tallies merge across calls", () => {
    const [w] = describeOculusCli(`oculus memory write a --text x`);
    expect(w.tally(1)).toBe("saved a memory");
    expect(w.tally(2)).toBe("saved 2 memories");
    const [g] = describeOculusCli(`oculus grep x`);
    const [s] = describeOculusCli(`oculus search y`);
    expect(g.key).toBe(s.key);
  });
});
