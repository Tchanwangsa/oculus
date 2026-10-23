import { useEffect, useState } from "react";
import { ArrowSquareOut } from "@phosphor-icons/react";
import { proposalInspect, type Proposal } from "@/lib/harness/proposals";
import { navigateActive } from "@/lib/shell/tabRouters";
import { openLibraryPath } from "@/lib/files/openFile";
import { taskHref } from "@/components/projects/nav/taskHref";
import { projectHref } from "@/components/projects/nav/projectHref";
import { object, type Row } from "./model";

export function Results({ proposal }: { proposal: Proposal }) {
  const [live, setLive] = useState<Record<string, Row | null>>({});
  const [error, setError] = useState<string>();
  const key = JSON.stringify(proposal.made);
  useEffect(() => {
    let active = true;
    const made = JSON.parse(key) as Proposal["made"];
    const groups = new Map<string, string[]>();
    for (const item of made) if (["task", "project", "event", "tag"].includes(item.entity)) groups.set(item.entity, [...(groups.get(item.entity) ?? []), item.id]);
    void Promise.all([...groups].map(async ([entity, ids]) => {
      const entries = await proposalInspect(entity, ids);
      return Object.fromEntries(Object.entries(entries).map(([id, value]) => [`${entity}:${id}`, value]));
    })).then((entries) => { if (active) setLive(Object.assign({}, ...entries)); }).catch((e) => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [key]);
  const open = (item: Proposal["made"][number]) => {
    const row = object(live[`${item.entity}:${item.id}`]);
    if (item.entity === "note") openLibraryPath(item.id);
    else if (item.entity === "task") navigateActive(taskHref(typeof row.project === "number" ? row.project : null, { id: Number(item.id), title: String(row.title ?? item.title ?? "Task") }));
    else if (item.entity === "project") navigateActive(projectHref({ id: Number(item.id), name: String(row.name ?? item.title ?? "Project") }));
    else if (item.entity === "event") navigateActive("/calendar");
    else if (item.entity === "tag") navigateActive(`/tasks?tag=${encodeURIComponent(item.id)}`);
  };
  return <div className="border-t border-border-subtle px-4 py-2">
    {proposal.made.map((item, i) => <div key={`${item.entity}:${item.id}:${i}`} className="flex min-w-0 items-center gap-2 py-1 text-[13px]">
      <span className="min-w-0 flex-1 break-words">{item.title ?? item.id}</span>
      {proposal.status === "created" && item.op !== "delete" && !proposal.kind.startsWith("Delete") && !(proposal.kind === "TagChanges" && Array.isArray(proposal.state.changes) && proposal.state.changes.some((change) => String(object(change).tag) === item.id)) && <button type="button" onClick={() => open(item)} className="inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-full px-2 py-1 text-xs text-brand hover:bg-brand-muted" aria-label={`Open ${item.title ?? item.id}`}>Open <ArrowSquareOut size={12} /></button>}
    </div>)}
    {error && <p className="text-xs text-destructive">Could not load current links: {error}</p>}
  </div>;
}
