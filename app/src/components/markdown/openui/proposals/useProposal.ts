import { useContext, useEffect, useMemo, useState } from "react";
import { useProposalStore } from "@/stores/chat/proposalStore";
import { proposalInspect } from "@/lib/harness/proposals";
import { WidgetFence, useWidgetRow } from "../state";
import { initialProposal, object, rowId, rowKey, rows, entityFor, type Row } from "./model";

export function useProposal(kind: string, input: Row) {
  const row = useWidgetRow();
  const block = useContext(WidgetFence);
  const id = row && row.row > 0 ? `${row.row}.${block}` : null;
  const source = JSON.stringify(input);
  const initial = useMemo(() => initialProposal(kind, JSON.parse(source)), [kind, source]);
  const proposal = useProposalStore((s) => id ? s.rows[id] : undefined);
  const draft = useProposalStore((s) => id ? s.drafts[id] : undefined);
  const busy = useProposalStore((s) => id ? !!s.busy[id] : false);
  const error = useProposalStore((s) => id ? s.errors[id] : undefined);
  useEffect(() => {
    if (!id || !row) return;
    void useProposalStore.getState().open(row.thread, row.row, block, kind, initial, typeof initial.amends === "string" ? initial.amends : null);
  }, [id, row?.thread, row?.row, block, kind, initial]);
  return {
    id, proposal, busy, error, initial, amended: proposal?.amended_from, state: draft ?? proposal?.state ?? initial,
    edit: (state: Row) => { if (id) useProposalStore.getState().edit(id, state); },
    act: (action: "apply" | "discard" | "undo" | "start") => { if (id) void useProposalStore.getState().act(id, action); },
    retry: () => { if (row) void useProposalStore.getState().open(row.thread, row.row, block, kind, initial, typeof initial.amends === "string" ? initial.amends : null); },
  };
}

/** Read live rows independently of writes, and refresh with the same window
 * events as planning views. Empty id lists never reach the database. */
export function useProposalLive(kind: string, state: Row) {
  const key = rowKey(kind);
  const entries = key ? rows(state[key]) : [];
  const ids = entries.map(rowId).filter((id): id is string | number => typeof id === "number" || typeof id === "string");
  const request = JSON.stringify({ entity: kind === "NewTasks" ? "task" : entityFor(kind), ids, project: kind === "NewTasks" ? state.project : null });
  const [data, setData] = useState<{ request: string; live: Record<string, Row | null>; project: Row | null; error?: string }>({ request: "", live: {}, project: null });
  useEffect(() => {
    let active = true;
    const query = JSON.parse(request) as { entity: string; ids: (string | number)[]; project: number | null };
    let run = 0;
    const load = async () => {
      const current = ++run;
      try {
        const [live, projects] = await Promise.all([
          query.ids.length ? proposalInspect(query.entity, query.ids) : Promise.resolve<Record<string, Row | null>>({}),
          query.project != null ? proposalInspect("project", [query.project]) : Promise.resolve<Record<string, Row | null>>({}),
        ]);
        if (active && current === run) setData({ request, live, project: query.project == null ? null : object(projects[String(query.project)]) });
      } catch (error) {
        if (active && current === run) setData({ request, live: {}, project: null, error: String(error) });
      }
    };
    void load();
    const refresh = () => void load();
    window.addEventListener("oculus:projects-updated", refresh);
    window.addEventListener("oculus:calendar-updated", refresh);
    return () => {
      active = false;
      window.removeEventListener("oculus:projects-updated", refresh);
      window.removeEventListener("oculus:calendar-updated", refresh);
    };
  }, [request]);
  return data.request === request ? { ...data, loaded: true } : { live: {} as Record<string, Row | null>, project: null, error: undefined, loaded: false };
}
