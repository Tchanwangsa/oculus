import { describe, expect, test } from "bun:test";
import { HIT_CLOSE, HIT_OPEN, snippetParts } from "@/lib/harness";

describe("snippetParts", () => {
  const hit = (t: string) => `${HIT_OPEN}${t}${HIT_CLOSE}`;

  test("splits hit pairs into hits and plain text", () => {
    expect(snippetParts(`…the ${hit("eigen")}values of ${hit("A")}…`)).toEqual([
      { text: "…the ", hit: false },
      { text: "eigen", hit: true },
      { text: "values of ", hit: false },
      { text: "A", hit: true },
      { text: "…", hit: false },
    ]);
  });

  test("a whole-title hit and plain text with no markers", () => {
    expect(snippetParts(hit("Bayes' rule"))).toEqual([{ text: "Bayes' rule", hit: true }]);
    expect(snippetParts("no hits here")).toEqual([{ text: "no hits here", hit: false }]);
    expect(snippetParts("")).toEqual([]);
  });

  test("a message's own **bold** is plain text, never a hit", () => {
    expect(snippetParts(`the **key** step is ${hit("pruning")}`)).toEqual([
      { text: "the key step is ", hit: false },
      { text: "pruning", hit: true },
    ]);
  });

  test("an unpaired marker is dropped", () => {
    expect(snippetParts(`a ${hit("b")} c ${HIT_OPEN} d`)).toEqual([
      { text: "a ", hit: false },
      { text: "b", hit: true },
      { text: " c  d", hit: false },
    ]);
    expect(snippetParts(`${HIT_OPEN}open`)).toEqual([{ text: "open", hit: false }]);
  });

  test("an empty pair draws nothing and never splits the text", () => {
    expect(snippetParts(`x${hit("")}y`)).toEqual([{ text: "xy", hit: false }]);
  });

  test("markup stays text: snippets are never HTML", () => {
    expect(snippetParts(`<b>${hit("x")}</b>`)).toEqual([
      { text: "<b>", hit: false },
      { text: "x", hit: true },
      { text: "</b>", hit: false },
    ]);
  });
});
