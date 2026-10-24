import { useEffect, useMemo, useState } from "react";
import { proposalModified, proposalVersions, readProposalVersions, restoreProposalVersion, type ProposalVersion } from "@/lib/harness/proposalVersions";
import { useProposalVersionStore } from "@/stores/chat/proposalVersionStore";
import type { Row } from "./model";
import type { useProposal } from "./useProposal";

export function useProposalVersions(kind: string, card: ReturnType<typeof useProposal>, onRestore: () => void) {
  const { id, proposal, initial, state } = card;
  const workspace = useProposalVersionStore((s) => id ? s.entries[id] : undefined);
  const [history, setHistory] = useState<{ id: string; versions: ProposalVersion[]; error?: string }>();
  const [attempt, setAttempt] = useState(0);
  const revision = workspace?.revision ?? 0;
  const thread = proposal?.thread_id;
  const next = proposal?.superseded_by;
  const amends = initial.amends;
  useEffect(() => {
    if (!id || thread == null) return;
    let live = true;
    readProposalVersions(thread).then(
      (versions) => { if (live) setHistory({ id, versions }); },
      (error) => { if (live) setHistory({ id, versions: [], error: String(error) }); },
    );
    return () => { live = false; };
  }, [id, thread, next, amends, attempt]);

  const versions = useMemo(() => proposalVersions(history?.id === id ? history.versions : [], {
    id: id ?? "preview", item: proposal?.item_id ?? 0, kind, state: initial, next: next ?? null,
  }), [history, id, proposal?.item_id, kind, initial, next]);
  const selected = versions.find((v) => v.id === workspace?.selected) ?? versions.find((v) => v.id === id) ?? versions[versions.length - 1];
  const writable = proposal?.status === "pending" && !card.busy;
  // A reset/switch unmounts the old editor. Ignore its last blur/unmount
  // commit, which belongs to the outgoing version rather than the new draft.
  const edit = (value: Row) => {
    const current = id ? useProposalVersionStore.getState().entries[id]?.revision ?? 0 : 0;
    if (current === revision) card.edit(value);
  };
  const restore = (value: Row) => {
    onRestore();
    card.edit(restoreProposalVersion(value, initial));
  };
  return {
    versions,
    selected,
    generation: revision,
    modified: proposalModified(state, selected.state),
    loading: !!id && (!proposal || history?.id !== id),
    error: history?.id === id ? history.error : undefined,
    retry: () => setAttempt((value) => value + 1),
    edit,
    select: (versionId: string) => {
      const version = versions.find((v) => v.id === versionId);
      if (!id || !proposal || !writable || !version || version.id === selected.id) return;
      const parked = useProposalVersionStore.getState().select(id, proposal.thread_id, proposal.item_id, version.id, state);
      restore(parked ?? version.state);
    },
    reset: () => {
      if (!id || !proposal || !writable) return;
      useProposalVersionStore.getState().reset(id, proposal.thread_id, proposal.item_id, selected.id);
      restore(selected.state);
    },
  };
}
