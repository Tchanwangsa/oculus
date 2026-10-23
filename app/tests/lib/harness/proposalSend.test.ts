import { expect, test } from "bun:test";
import { createHarnessMessaging } from "@/lib/harness/commands";
import { createProposalStore } from "@/stores/chat/proposalStore";
import type { Proposal, ProposalApi } from "@/lib/harness/proposals";

const proposal: Proposal = { id: "71.0", item_id: 71, block: 0, thread_id: 7, kind: "NewTasks", status: "pending", state: { project: null, items: [{ title: "Original" }] }, told: null, updated_at: "2026-10-24", made: [] };
const api = (edit: ProposalApi["edit"]): ProposalApi => ({ open: async () => proposal, edit, apply: async () => proposal, discard: async () => proposal, undo: async () => proposal, start: async () => proposal });

for (const operation of ["send", "edit-resend"] as const) test(`${operation} flushes edits made immediately before the message`, async () => {
  let saved = false;
  const calls: string[] = [];
  const store = createProposalStore(api(async (_id, state) => { calls.push("proposal_edit"); await Promise.resolve(); saved = true; return { ...proposal, state }; }), 60_000);
  const sender = createHarnessMessaging(async <T,>(command: string): Promise<T> => { calls.push(command); expect(saved).toBe(true); return 7 as T; }, (thread) => store.getState().flushThread(thread));
  store.getState().receive(proposal);
  store.getState().edit(proposal.id, { project: null, items: [{ title: "The student's latest edit" }] });
  if (operation === "send") await sender.send(7, "codex", "Make it due Friday", {});
  else await sender.editResend(7, 70, "Make it due Friday", {});
  expect(calls).toEqual(["proposal_edit", operation === "send" ? "harness_send" : "harness_edit_resend"]);
});

test("a failed card save prevents sending stale proposal context", async () => {
  let sent = false;
  const store = createProposalStore(api(async () => { throw new Error("database busy"); }), 60_000);
  const sender = createHarnessMessaging(async <T,>(): Promise<T> => { sent = true; return undefined as T; }, (thread) => store.getState().flushThread(thread));
  store.getState().receive(proposal);
  store.getState().edit(proposal.id, { project: null, items: [{ title: "Unsaved edit" }] });
  await expect(sender.send(7, "codex", "Amend this", {})).rejects.toThrow("database busy");
  expect(sent).toBe(false);
  store.getState().prune(7);
});
