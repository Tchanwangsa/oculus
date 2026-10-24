import { expect, test } from "bun:test";
import { proposalModified, proposalVersions, restoreProposalVersion, type ProposalVersion } from "@/lib/harness/proposalVersions";

const version = (id: string, amends?: string, next: string | null = null, kind = "NewCheatNotes"): ProposalVersion => ({ id, item: Number(id.split('.')[0]), kind, state: { notes: [{ title: id, body: "Original" }], ...(amends ? { amends } : {}) }, next });

test("version numbers follow only a reciprocal, chronological amendment chain", () => {
  const first = version("10.0", undefined, "20.0"), second = version("20.0", "10.0", "30.0"), third = version("30.0", "20.0");
  const unrelated = version("15.0");
  expect(proposalVersions([first, second, third, unrelated], third).map((v) => v.id)).toEqual(["10.0", "20.0", "30.0"]);
  expect(proposalVersions([first, second, third, unrelated], first).map((v) => v.id)).toEqual(["10.0", "20.0", "30.0"]);
  expect(proposalVersions([{ ...first, next: "15.0" }, second], second).map((v) => v.id)).toEqual(["20.0"]);
  expect(proposalVersions([first, { ...second, kind: "NewTasks" }], first).map((v) => v.id)).toEqual(["10.0"]);
});

test("reset restores content, inclusion and deleted rows without moving the active card's amendment link", () => {
  const first = version("10.0").state, current = version("30.0", "20.0").state;
  const restored = restoreProposalVersion(first, current);
  expect(restored).toEqual({ notes: [{ title: "10.0", body: "Original" }], amends: "20.0" });
  expect(proposalModified({ ...restored, notes: [] }, first)).toBe(true);
  expect(proposalModified(restored, first)).toBe(false);
  expect(proposalModified({ ...restored, notes: [{ title: "10.0", body: "Original", included: false }] }, first)).toBe(true);
  expect(restoreProposalVersion(current, first)).not.toHaveProperty("amends");
});

test("Modified ignores object property order and amendment identity but retains row order and all content edits", () => {
  expect(proposalModified({ notes: [{ body: "Text", title: "A" }], amends: "2.0" }, { amends: "1.0", notes: [{ title: "A", body: "Text" }] })).toBe(false);
  expect(proposalModified({ notes: [{ title: "B" }, { title: "A" }] }, { notes: [{ title: "A" }, { title: "B" }] })).toBe(true);
});
