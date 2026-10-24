import { useEffect, useMemo, useState } from "react";
import { ArrowDown, ArrowCounterClockwise, CalendarBlank, Check, CheckSquare, FileText, FolderSimple, SpinnerGap, Tag } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { DEFAULT_COLUMNS } from "@/lib/planning/projects/columns";
import { useWidgetProgress } from "../progress";
import { Fields } from "./Fields";
import { Pager, dateShift, displayValue, ChangedSince } from "./ProposalParts";
import { CreationRow, ChangeRow, NoteProposal, replaceAt, viewRows } from "./ProposalRows";
import { Results } from "./Results";
import { CheatRow } from "./CheatRow";
import { useProposal, useProposalLive } from "./useProposal";
import { useProposalVersions } from "./useProposalVersions";
import { VersionControl } from "./VersionControl";
import { conflicts, effectiveChanges, included, object, rowId, liveKey, rowKey, rows, validateProposal, type Row } from "./model";
import type { Proposal } from "@/lib/harness/proposals";

const plural = (count: number, noun: string) => `${count} ${count === 1 ? noun : noun.replace(/([^aeiou])y$/, "$1ie") + "s"}`;

function resultTitle(proposal: Proposal, project: Row | null): string {
  const { kind, made, state } = proposal;
  const tasks = made.filter((item) => item.entity === "task").length;
  let title;
  if (kind === "NewCheatNotes") title = `Added ${plural(made.filter((item) => item.entity === "cheat").length, "cheat sheet entry")}`;
  else if (kind === "EditItems") title = `Updated ${plural(made.length, "item")}`;
  else if (kind === "NewTasks") title = rows(state.items).every((item) => item.task != null) ? `Added ${plural(tasks, "subtask")}` : `Created ${plural(tasks, "task")}${project?.name ? ` in ${project.name}` : ""}`;
  else if (kind === "NewProject") title = `Created project “${state.name}” with ${plural(tasks, "task")}`;
  else if (kind === "NewEvents") title = `Created ${plural(made.filter((item) => item.entity === "event").length, "event")}`;
  else if (kind === "NewNote") title = `Created note “${state.title}”`;
  else if (kind.startsWith("Delete")) title = `Deleted ${plural(made.length, kind === "DeleteTasks" ? "task" : "event")}`;
  else title = `Updated ${plural(made.length, kind === "TaskChanges" ? "task" : kind === "ProjectChanges" ? "project" : kind === "EventChanges" ? "event" : "tag")}`;
  return proposal.status === "undone" ? `Undone · ${title}` : title;
}

function headline(kind: string, state: Row, count: number, total: number, project: Row | null, live: Record<string, Row | null>): string {
  if (kind === "NewCheatNotes") return `Add ${plural(total, "entry")} to your cheat sheet?`;
  if (kind === "EditItems") return `Edit ${plural(total, "item")}?`;
  if (kind === "NewTasks") {
    const under = rows(state.items).filter((item) => item.task != null);
    if (under.length && under.length === rows(state.items).length) return `Add ${plural(total, "subtask")} under ${plural(under.length, "task")}?`;
    if (under.length) return `Create ${total} tasks and subtasks?`;
    if (total === 1) return `Create task “${rows(state.items)[0]?.title}”?`;
    return `Create ${plural(total, "task")}${project?.name ? ` in ${project.name}` : ""}?`;
  }
  if (kind === "NewProject") return `Create project “${state.name}” with ${plural(total, "task")}?`;
  if (kind === "NewEvents") return total === 1 ? `Create event “${rows(state.events)[0]?.title}”?` : `Create ${plural(total, "event")}?`;
  if (kind === "NewNote") return `Create note “${state.title}”?`;
  if (kind.startsWith("Delete")) {
    const items = rows(state[kind === "DeleteTasks" ? "tasks" : "events"]), title = live[String(rowId(items[0] ?? {}))]?.title;
    return total === 1 && title ? `Delete “${title}”?` : `Delete ${plural(total, kind === "DeleteTasks" ? "task" : "event")}?`;
  }
  const changes = rows(state.changes);
  for (const field of ["due", "starts", "start"]) {
    const shifts = changes.map((change) => Object.keys(object(change.to)).length === 1 ? dateShift(object(change.from)[field], object(change.to)[field]) : null);
    if (total > 1 && shifts.length && shifts[0] && shifts.every((shift) => shift === shifts[0])) {
      const backwards = shifts[0].startsWith("+");
      return `${backwards ? "Push" : "Bring"} ${plural(total, field === "due" ? "due date" : "start date")} ${backwards ? "back" : "forward"} ${shifts[0].replace(/^[+-]/, "")}?`;
    }
  }
  if (kind === "TaskChanges" && changes.length && changes.every((item) => Object.keys(object(item.to)).length === 1 && columns(live[String(rowId(item))]).some((column) => column.id === object(item.to).column && column.kind === "done"))) {
    const title = changes.length === 1 ? live[String(rowId(changes[0]))]?.title : null;
    return title ? `Mark “${title}” done?` : `Mark ${plural(total, "task")} done?`;
  }
  if (changes.length === 1) {
    const change = changes[0], fields = Object.keys(object(change.to)), liveRow = live[String(rowId(change))];
    if (fields.length === 1 && liveRow) return `Change “${liveRow.title ?? liveRow.name}” ${fields[0]} to ${displayValue(object(change.to)[fields[0]], fields[0])}?`;
  }
  if (kind === "ProjectChanges" && changes.every((item) => object(item.to).status === "archived")) return `Archive ${plural(total, "project")}?`;
  return `Apply ${plural(count, kind === "TagChanges" ? "tag change" : "change")}?`;
}

function columns(row: Row | null | undefined): Row[] {
  let values = row?.columns;
  if (typeof values === "string") { try { values = JSON.parse(values); } catch { values = null; } }
  return Array.isArray(values) ? values.map(object) : DEFAULT_COLUMNS.map((column) => ({ ...column }));
}

/** The same persisted decision frame surrounds every student-owned write. */
export function ProposalCard({ kind, input }: { kind: string; input: Row }) {
  const card = useProposal(kind, input);
  validateProposal(kind, card.initial);
  const { state, proposal } = card;
  const key = rowKey(kind);
  const data = useProposalLive(kind, state);
  const progress = useWidgetProgress(0);
  const [validation, setValidation] = useState<string>();
  const version = useProposalVersions(kind, card, () => setValidation(undefined));
  const [expanded, setExpanded] = useState(false);
  const listing = useMemo(() => key ? viewRows(kind, state, key).filter((item) => !item.row.from || Object.keys(effectiveChanges(item.row)).length > 0) : [], [kind, state, key]);
  const page = Math.min(Math.max(0, Number(progress.value) || 0), Math.max(0, Math.ceil(listing.length / 5) - 1));
  const pending = !proposal || proposal.status === "pending";
  const disabled = card.busy || !proposal || !pending;
  const active = listing.filter((item) => item.allowed && included(item.row) && (!item.row.from || Object.keys(effectiveChanges(item.row)).length) && (item.row.included !== undefined || !data.loaded || !conflicts(item.row, data.live[liveKey(item.row)]).length));
  const count = kind === "NewNote" ? 1 : active.length;
  const total = listing.length;
  const Icon = kind.includes("Project") ? FolderSimple : kind.includes("Event") ? CalendarBlank : kind.includes("Note") ? FileText : kind.includes("Tag") ? Tag : CheckSquare;
  const title = headline(kind, state, count, total, data.project, data.live);
  const onlyUnder = kind === "NewTasks" && rows(state.items).length > 0 && rows(state.items).every((item) => item.task != null);
  const applyLabel = kind === "NewCheatNotes" ? `Add ${plural(count, "entry")}` : kind === "NewProject" ? `Create project${count ? ` and ${plural(count, "task")}` : ""}` : kind === "NewNote" ? "Create note" : kind === "NewTasks" ? onlyUnder ? `Add ${plural(count, "subtask")}` : `Create ${plural(count, "task")}` : kind === "NewEvents" ? `Create ${plural(count, "event")}` : kind.startsWith("Delete") ? `Delete ${plural(count, kind === "DeleteTasks" ? "task" : "event")}` : `Apply ${count} of ${total}`;
  const change = (next: Row) => { setValidation(undefined); version.edit(next); };
  // A stale row defaults to excluded in the durable state, so every send and
  // a second view see the same decision. An explicit tick rebases its from.
  useEffect(() => {
    if (!key || !pending || !proposal || !data.loaded || data.error) return;
    const values = rows(state[key]);
    const next = values.map((row) => row.from && row.included === undefined && conflicts(row, data.live[liveKey(row)]).length ? { ...row, included: false } : row);
    if (next.some((row, i) => row !== values[i])) card.edit({ ...state, [key]: next });
  }, [state, proposal, key, data.loaded, data.error, data.live, pending]);

  const apply = () => {
    try {
      let next = state;
      if (key && (kind.endsWith("Changes") || kind === "EditItems")) next = { ...state, [key]: rows(state[key]).map((row) => row.from && !Object.keys(effectiveChanges(row)).length ? { ...row, included: false } : row) };
      validateProposal(kind, next);
      if (next !== state) card.edit(next);
      card.act("apply");
    } catch (error) { setValidation(String(error)); }
  };
  const anchor = card.id ? `proposal-${card.id}` : undefined;
  const small = kind !== "EditItems" && listing.length === 1 && !listing[0].parent && !["NewProject", "NewNote", "NewCheatNotes", "TagChanges"].includes(kind) && (!listing[0].row.from || Object.keys(object(listing[0].row.to)).length === 1) && !Object.keys(object(listing[0].row.to)).some((field) => ["body", "brief", "notes"].includes(field));
  if (small && !expanded && pending) return <section id={anchor} aria-label="Proposal" className="@container/proposal my-3 min-w-0 rounded-xl border border-border bg-card">
    <div className="grid grid-cols-[auto_minmax(0,1fr)] items-start gap-2 px-4 py-3 @sm/proposal:grid-cols-[auto_minmax(0,1fr)_auto] @sm/proposal:items-center">
      <Icon size={16} className="mt-0.5 shrink-0 text-brand @sm/proposal:mt-0" />
      <div className="flex min-w-0 items-center gap-1.5"><button type="button" disabled={disabled} onClick={() => setExpanded(true)} aria-label="Edit proposal" className="min-w-0 flex-1 cursor-pointer text-left text-[13px] break-words hover:text-brand">{title}</button><ChangedSince fields={data.loaded ? conflicts(listing[0].row, data.live[liveKey(listing[0].row)]) : []} live={data.live[String(rowId(listing[0].row))]} /></div>
      <div className="col-span-2 flex justify-end gap-2 @sm/proposal:col-span-1"><Button variant="ghost" size="sm" disabled={disabled} onClick={() => card.act("discard")}>Discard</Button><Button variant={kind.startsWith("Delete") ? "destructive" : "default"} size="sm" disabled={disabled || count === 0} onClick={apply}>{card.busy && <SpinnerGap className="animate-spin" />}{kind.startsWith("New") ? "Create" : kind.startsWith("Delete") ? "Delete" : "Apply"}</Button></div>
    </div>
    {pending && <div className="px-4 pb-2"><VersionControl version={version} disabled={disabled} /></div>}
    {(card.error || validation || data.error) && <p role="alert" className="px-4 pb-2 text-xs text-destructive">{card.error ?? validation ?? data.error}</p>}
  </section>;
  if (proposal && ["discarded", "superseded"].includes(proposal.status)) return <div id={anchor} className="my-3 flex flex-wrap items-center gap-2 rounded-xl border border-border px-4 py-3 text-[13px] text-muted-foreground">
    {proposal.status === "superseded" ? <><ArrowDown size={14} /><span>V{version.versions.findIndex((item) => item.id === card.id) + 1}</span>{proposal.superseded_by && <a className="text-brand hover:underline" href={`#proposal-${proposal.superseded_by}`} onClick={(event) => { event.preventDefault(); document.getElementById(`proposal-${proposal.superseded_by}`)?.scrollIntoView({ behavior: "smooth", block: "center" }); }}>View newer version</a>}</> : <span>Discarded · {title.replace(/\?$/, "")}</span>}
  </div>;

  return <section id={anchor} aria-label="Proposal" className="@container/proposal my-3 min-w-0 overflow-hidden rounded-xl border border-border bg-card">
    <header className="flex min-w-0 items-start gap-2.5 px-4 py-3">
      <Icon size={18} className="mt-0.5 shrink-0 text-brand" />
      <div className="min-w-0 flex-1">
        {pending && kind === "NewProject" ? <div className="font-display text-[13px] leading-7 font-semibold">Create project “<Input aria-label="Project name" disabled={disabled} value={String(state.name ?? "")} onChange={(event) => change({ ...state, name: event.target.value })} className="inline h-7 max-w-full border-0 bg-transparent p-0 text-[13px] font-semibold shadow-none" style={{ width: `${Math.max(4, String(state.name ?? "").length)}ch` }} />” with {plural(total, "task")}?</div> : <div role="heading" aria-level={4} className="font-display text-[13px] leading-relaxed font-semibold break-words">{pending ? title : resultTitle(proposal!, data.project)}</div>}
        {data.project?.subject != null && <p className="mt-0.5 text-xs text-muted-foreground">{String(data.project.subject)}</p>}
        {pending && <VersionControl version={version} disabled={disabled} />}
      </div>
      {pending && total > 1 && <span className="mt-0.5 shrink-0 rounded-full bg-surface px-2 py-0.5 text-xs tabular-nums text-muted-foreground">{count} of {total}</span>}
    </header>
    {pending ? <>
      {kind === "NewProject" && <fieldset key={`project:${version.generation}`} disabled={disabled} className="border-t border-border-subtle px-4"><Fields row={state} names={["subject", "due", "starts", "tags", "event", "brief"]} onChange={change} /></fieldset>}
      {kind === "NewNote" && <NoteProposal key={`note:${version.generation}`} state={state} disabled={disabled} onChange={change} />}
      {listing.slice(page * 5, page * 5 + 5).map((item, visibleIndex, visible) => {
        const live = data.live[item.row.entity ? liveKey(item.row) : String(rowId(item.row) ?? item.parent?.task)];
        const previousRows = key && card.amended ? viewRows(kind, card.amended, key) : [];
        const previous = previousRows.find((old) => (rowId(item.row) != null && liveKey(old.row) === liveKey(item.row)) || (item.row.title != null && old.row.title === item.row.title))?.row ?? previousRows.find((old) => JSON.stringify(old.path) === JSON.stringify(item.path))?.row;
        const edit = (row: Row) => change(replaceAt(state, item.path, row));
        const editorKey = `${version.generation}:${item.path.join(".")}`;
        if (kind === "NewCheatNotes") return <CheatRow key={editorKey} row={item.row} allowed={item.allowed} previous={card.amended ? previous : undefined} topics={data.topics} disabled={disabled} onChange={edit} />;
        return kind.startsWith("New") ? <CreationRow key={editorKey} kind={kind} item={{ ...item, showParent: visibleIndex === 0 || JSON.stringify(visible[visibleIndex - 1].parentPath) !== JSON.stringify(item.parentPath) }} live={live} previous={card.amended ? previous : undefined} columns={columns(item.parent?.task != null ? live : data.project)} topics={data.topics} disabled={disabled} onChange={edit} /> : <ChangeRow key={editorKey} kind={kind} row={item.row} previous={card.amended ? previous : undefined} live={live} topics={data.topics} loaded={data.loaded} columns={columns(live)} disabled={disabled} onChange={edit} />;
      })}
      <Pager page={page} count={total} onChange={(next) => progress.setValue(next)} />
    </> : <Results proposal={proposal!} />}
    {(card.error || validation || data.error) && <p role="alert" className="border-t border-border-subtle px-4 py-2 text-xs leading-relaxed text-destructive">{card.error ?? validation ?? data.error}</p>}
    <footer className={cn("flex flex-wrap items-center justify-end gap-2 border-t border-border-subtle bg-surface/40 px-4", pending ? "py-3" : "py-2")}>
      {!card.id && <span className="mr-auto text-xs text-muted-foreground">Available after the reply finishes.</span>}
      {card.id && !proposal && <span className="mr-auto text-xs text-muted-foreground">{card.error ? <Button variant="ghost" size="xs" onClick={card.retry}>Try loading again</Button> : "Loading card…"}</span>}
      {pending ? <><Button variant="ghost" size="sm" disabled={disabled} onClick={() => card.act("discard")}>Discard</Button><Button variant={kind.startsWith("Delete") ? "destructive" : "default"} size="sm" disabled={disabled || (count === 0 && kind !== "NewProject")} onClick={apply}>{card.busy && <SpinnerGap className="animate-spin" />}{applyLabel}</Button></> : proposal?.status === "created" ? <><Check size={14} className="mr-auto text-success" aria-label="Applied" /><Button variant="outline" size="xs" disabled={card.busy} onClick={() => card.act("undo")}><ArrowCounterClockwise />Undo</Button></> : <span className="mr-auto text-xs text-muted-foreground">Changes undone</span>}
    </footer>
  </section>;
}
