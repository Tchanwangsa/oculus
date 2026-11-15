import { describe, expect, test } from "bun:test";
import { heldDescription, threadBannerItems, type ThreadBannerInputs } from "@/components/harness/composer/ThreadBanners";

const base: ThreadBannerInputs = { items: [], windows: [], dismissed: [], onDismiss: () => {}, now: 0 };
const resume = (patch: Partial<NonNullable<ThreadBannerInputs["resume"]>> = {}) => ({
  onClick: () => {},
  pending: false,
  error: null,
  ...patch,
});
const held = [
  { id: "1", text: "first", held: true },
  { id: "2", text: "second", held: true },
];

describe("threadBannerItems: a held queue", () => {
  test("an idle chat with held messages offers Resume", () => {
    const [b] = threadBannerItems({ ...base, queued: held, resume: resume() });
    expect(b.id).toBe("held");
    expect(b.title).toBe("2 messages waiting");
    expect(b.actions?.map((a) => [a.label, a.primary])).toEqual([["Resume", true]]);
  });

  test("one message is one message", () => {
    const [b] = threadBannerItems({ ...base, queued: [held[0]], resume: resume() });
    expect(b.title).toBe("1 message waiting");
  });

  test("after a failed reply it says so", () => {
    const [b] = threadBannerItems({ ...base, queued: held, lastRun: "failed", resume: resume() });
    expect(b.description).toBe(heldDescription("failed"));
    expect(b.description).toStartWith("The last reply failed");
  });

  test("a stop and a restart cannot be told apart, so they share the words", () => {
    for (const lastRun of ["interrupted", "completed", null] as const) {
      const [b] = threadBannerItems({ ...base, queued: held, lastRun, resume: resume() });
      expect(b.description).toStartWith("Held after a stop or a restart.");
    }
  });

  test("nothing while a turn is open, nothing for messages that are not held", () => {
    expect(threadBannerItems({ ...base, queued: held, running: true, resume: resume() })).toEqual([]);
    const waiting = held.map((q) => ({ ...q, held: false }));
    expect(threadBannerItems({ ...base, queued: waiting, resume: resume() })).toEqual([]);
  });

  test("no Resume to offer, no notice", () => {
    expect(threadBannerItems({ ...base, queued: held })).toEqual([]);
  });

  test("while Resume is out its button is disabled; a failure is said in the card", () => {
    const [out] = threadBannerItems({ ...base, queued: held, resume: resume({ pending: true }) });
    expect(out.actions?.[0].disabled).toBe(true);
    const [failed] = threadBannerItems({ ...base, queued: held, resume: resume({ error: "no such thread" }) });
    expect(failed.description).toBe("Couldn't resume: no such thread");
    expect(failed.tone).toBe("danger");
    expect(failed.actions?.[0].disabled).toBe(false);
  });

  test("clicking Resume calls it", () => {
    let n = 0;
    threadBannerItems({ ...base, queued: held, resume: resume({ onClick: () => n++ }) })[0].actions?.[0].onClick();
    expect(n).toBe(1);
  });
});
