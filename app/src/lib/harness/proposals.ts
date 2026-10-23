import { invoke } from "@tauri-apps/api/core";

export type ProposalState = Record<string, unknown>;
export type ProposalStatus = "pending" | "created" | "discarded" | "superseded" | "undone";
export interface Proposal {
  id: string;
  item_id: number;
  block: number;
  thread_id: number;
  kind: string;
  status: ProposalStatus;
  state: ProposalState;
  told: string | null;
  updated_at: string;
  made: { entity: string; id: string; title?: string; op?: string }[];
  superseded_by?: string;
  amended_from?: ProposalState;
}

export interface ProposalApi {
  open: (threadId: number, itemId: number, block: number, kind: string, state: ProposalState, amends?: string | null) => Promise<Proposal>;
  edit: (id: string, state: ProposalState) => Promise<Proposal>;
  apply: (id: string, state: ProposalState) => Promise<Proposal>;
  discard: (id: string) => Promise<Proposal>;
  undo: (id: string) => Promise<Proposal>;
  start: (id: string) => Promise<Proposal>;
}

export const proposalApi: ProposalApi = {
  open: (threadId, itemId, block, kind, state, amends) => invoke("proposal_open", { threadId, itemId, block, kind, state, amends: amends ?? null }),
  edit: (id, state) => invoke("proposal_edit", { id, state }),
  apply: (id, state) => invoke("proposal_apply", { id, state }),
  discard: (id) => invoke("proposal_discard", { id }),
  undo: (id) => invoke("proposal_undo", { id }),
  start: (id) => invoke("proposal_job_start", { id }),
};

export function proposalJobInfo(kind: string, state: ProposalState): Promise<{ description: string }> {
  return invoke("proposal_job_info", { kind, state });
}

export function proposalInspect(entity: string, ids: (string | number)[]): Promise<Record<string, ProposalState | null>> {
  return invoke("proposal_inspect", { entity, ids });
}
