import { afterAll, describe, expect, test } from "bun:test";

// Card components reach modules that listen on `window` and `document` at load.
const globals = globalThis as { window?: unknown; document?: unknown };
const stubbed = (["window", "document"] as const).filter((k) => !(k in globals));
for (const k of stubbed) globals[k] = Object.assign(new EventTarget(), { documentElement: { style: {} } });
afterAll(() => {
  for (const k of stubbed) delete globals[k];
});
const { parseProgram } = await import("@/components/markdown/openui/parse");
const { generateOculusUiPrompt } = await import("@/components/markdown/openui/prompt");
const { initialProposal } = await import("@/components/markdown/openui/proposals/model");
const { cardState } = await import("@/components/markdown/openui/proposals/cardState");
const { continueLabel, followUp, followUpNote } = await import("@/components/markdown/openui/proposals/followUp");

const THEN = "Check the new Ed threads for extension posts";

/** The one card a fence holds, as its component's props. */
function card(source: string): Record<string, unknown> {
  const verdict = parseProgram(`root = Stack([${source}])`);
  if (!verdict.ok) throw new Error(verdict.errors.join("; "));
  const items = verdict.result.root?.props.items as { props: Record<string, unknown> }[];
  return items[0].props;
}

describe("then grammar", () => {
  const withThen = [
    `Sync(["COMP30026"], null, "${THEN}")`,
    `Reparse(["courses/COMP30026/Week1.pdf"], "${THEN}")`,
    `Transcribe("courses/COMP30026/Week1.mp4", "${THEN}")`,
    `LectureChapters("lecture-1", "${THEN}")`,
    `LectureEnd("lecture-1", "${THEN}")`,
    `Download("aligner", "${THEN}")`,
    `NewTasks(null, [Task("Outline")], null, "${THEN}")`,
    `NewNote("COMP30026", "Summary", "The body", null, "${THEN}")`,
    `TaskChanges([Change(42, {due: "2026-10-27"}, {due: "2026-10-30"})], null, "${THEN}")`,
    `DeleteTasks([1, 2], "812.0", "${THEN}")`,
  ];
  for (const source of withThen) test(`${source.split("(")[0]} takes then as its last argument`, () => {
    expect(card(source).then).toBe(THEN);
  });

  test("then is optional, and amends keeps its place before it", () => {
    expect(card(`Sync(["COMP30026"])`).then ?? null).toBeNull();
    const amended = card(`DeleteTasks([1], "812.0", "${THEN}")`);
    expect(amended.amends).toBe("812.0");
    expect(amended.then).toBe(THEN);
  });

  test("then reaches the card's stored state", () => {
    expect(initialProposal("Sync", card(`Sync(["COMP30026"], null, "${THEN}")`)).then).toBe(THEN);
  });

  test("the generated prompt names then on every card and shows one example", () => {
    const prompt = generateOculusUiPrompt();
    expect(prompt).toContain("Sync(subjects: string[], lectures?: boolean, then?: string)");
    expect(prompt).toContain("DeleteEvents(events: string[], amends?: string, then?: string)");
    expect(prompt).toContain(`Sync(["COMP30026"], null, "${THEN}")`);
    expect(prompt).not.toContain("always the last argument");
  });
});

describe("follow-up buttons and notes", () => {
  const running = cardState({ streaming: false, id: "10.0", busy: false, status: "created", job: "running" });
  const done = cardState({ streaming: false, id: "10.0", busy: false, status: "created", job: "complete" });

  test("a card with a follow-up says so on its deciding buttons", () => {
    const follow = followUp({ subjects: ["X"], then: THEN });
    expect(continueLabel("Sync", follow)).toBe("Sync and continue");
    expect(continueLabel("Create 3 tasks", follow)).toBe("Create 3 tasks and continue");
    expect(continueLabel("Discard", follow)).toBe("Discard and continue");
  });

  test("without one, or with a blank one, the buttons are unchanged and no note shows", () => {
    for (const state of [{}, { then: "   " }, { then: 5 }]) {
      const follow = followUp(state);
      expect(continueLabel("Sync", follow)).toBe("Sync");
      expect(followUpNote(follow, running)).toBeUndefined();
    }
  });

  test("the note follows the claim Rust records", () => {
    const note = (continuation?: unknown, frame = done) => followUpNote(followUp({ then: THEN, continuation }), frame);
    expect(note(undefined, running)).toBe("The agent continues when this finishes.");
    expect(note(undefined)).toBeUndefined();
    expect(note({ phase: "queued" })).toBe("The agent continues after its current reply.");
    expect(note({ phase: "started", run_id: 4 })).toBe("The agent continued below.");
    expect(note({ phase: "skipped", reason: "Stopped before the agent continued." })).toBe("Stopped before the agent continued.");
  });
});
