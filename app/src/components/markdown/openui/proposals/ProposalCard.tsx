import { useContext, useEffect, useMemo, useState } from "react";
import { OpenUISkeleton, StreamingPending } from "../Streaming";
import { ArrowClockwise, ArrowDown, ArrowCounterClockwise, CalendarBlank, CheckSquare, FileText, FolderSimple, SpinnerGap, Tag } from "@phosphor-icons/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { ViewTabs } from "@/components/ui/table/ViewTabs";
import { DEFAULT_COLUMNS } from "@/lib/planning/projects/columns";
import { useWidgetProgress } from "../progress";
import { ProjectAttributes } from "./ProjectAttributes";
import { Pager, dateShift, displayValue, ChangedSince } from "./ProposalParts";
import { CreationRow, ChangeRow, NoteProposal, replaceAt, viewRows } from "./ProposalRows";
import { Results } from "./Results";
import { CheatRow } from "./CheatRow";
import { useProposal, useProposalLive } from "./useProposal";
import { useProposalVersions } from "./useProposalVersions";
import { VersionControl } from "./VersionControl";
import { CardFrame, useCardStreaming } from "./CardFrame";
import { cardState } from "./cardState";
import { conflicts, effectiveChanges, excludeStale, included, object, rowId, liveKey, rowKey, rows, validateProposal, type Row } from "./model";
import type { Proposal } from "@/lib/harness/proposals";
import { continueLabel, followUp, followUpNote } from "./followUp";

type Action = "apply" | "discard" | "undo";

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
  return title;
}

function headline(kind: string, state: Row, total: number, project: Row | null, live: Record<string, Row | null>): string {
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
  return `Apply ${plural(total, kind === "TagChanges" ? "tag change" : "change")}?`;
}

function columns(row: Row | null | undefined): Row[] {
  let values = row?.columns;
  if (typeof values === "string") { try { values = JSON.parse(values); } catch { values = null; } }
  return Array.isArray(values) ? values.map(object) : DEFAULT_COLUMNS.map((column) => ({ ...column }));
}

/** Every student-owned write draws in the shared `CardFrame`; `cardState`
 * names its phase, and only `pending` (or a refused Apply) takes input. */
export function ProposalCard({ kind, input }: { kind: string; input: Row }) {
  const streaming = useCardStreaming();
  const pending = useContext(StreamingPending);
  if (streaming && pending) return <OpenUISkeleton title={`Drawing ${kind}…`} />;
  return <ProposalCardContent kind={kind} input={input} />;
}

function ProposalCardContent({ kind, input }: { kind: string; input: Row }) {
  const card = useProposal(kind, input);
  const streaming = useCardStreaming();
  if (!streaming) validateProposal(kind, card.initial);
  const { state, proposal } = card;
  const key = rowKey(kind);
  const data = useProposalLive(kind, state);
  const progress = useWidgetProgress(0);
  const [validation, setValidation] = useState<string>();
  const [acting, setActing] = useState<Action>("apply");
  const version = useProposalVersions(kind, card, () => setValidation(undefined));
  const [expanded, setExpanded] = useState(false);
  const [tab, setTab] = useState<"attributes" | "tasks">("attributes");
  const frame = cardState({ streaming, id: card.id, status: proposal?.status, busy: card.busy, error: card.error ?? validation });
  const listing = useMemo(() => key ? viewRows(kind, state, key).filter((item) => !item.row.from || Object.keys(effectiveChanges(item.row)).length > 0) : [], [kind, state, key]);
  const page = Math.min(Math.max(0, Number(progress.value) || 0), Math.max(0, Math.ceil(listing.length / 5) - 1));
  const pending = proposal?.status === "pending";
  /** The proposed rows show until a decision; afterwards the results do. */
  const editor = !proposal || pending;
  const disabled = !frame.editable;
  const active = listing.filter((item) => item.allowed && included(item.row) && (!item.row.from || Object.keys(effectiveChanges(item.row)).length) && (item.row.included !== undefined || !data.loaded || !conflicts(item.row, data.live[liveKey(item.row)]).length));
  const count = kind === "NewNote" ? 1 : active.length;
  const total = listing.length;
  const Icon = kind.includes("Project") ? FolderSimple : kind.includes("Event") ? CalendarBlank : kind.includes("Note") ? FileText : kind.includes("Tag") ? Tag : CheckSquare;
  const title = headline(kind, state, total, data.project, data.live);
  const onlyUnder = kind === "NewTasks" && rows(state.items).length > 0 && rows(state.items).every((item) => item.task != null);
  const applyLabel = kind === "NewCheatNotes" ? `Add ${plural(count, "entry")}` : kind === "NewProject" ? `Create project${count ? ` and ${plural(count, "task")}` : ""}` : kind === "NewNote" ? "Create note" : kind === "NewTasks" ? onlyUnder ? `Add ${plural(count, "subtask")}` : `Create ${plural(count, "task")}` : kind === "NewEvents" ? `Create ${plural(count, "event")}` : kind.startsWith("Delete") ? `Delete ${plural(count, kind === "DeleteTasks" ? "task" : "event")}` : `Apply ${count} of ${total}`;
  const change = (next: Row) => { setValidation(undefined); version.edit(next); };
  const act = (action: Action) => { setActing(action); card.act(action); };
  // A stale row defaults to excluded in the durable state, so every send and
  // a second view see the same decision. An explicit tick rebases its from.
  useEffect(() => {
    if (!key || !pending || !data.loaded || data.error) return;
    const values = rows(state[key]);
    const next = excludeStale(values, data.live);
    if (next !== values) card.edit({ ...state, [key]: next });
  }, [state, key, data.loaded, data.error, data.live, pending]);

  const apply = () => {
    try {
      let next = state;
      if (key && (kind.endsWith("Changes") || kind === "EditItems")) next = { ...state, [key]: rows(state[key]).map((row) => row.from && !Object.keys(effectiveChanges(row)).length ? { ...row, included: false } : row) };
      validateProposal(kind, next);
      if (next !== state) card.edit(next);
      act("apply");
    } catch (error) { setValidation(String(error)); }
  };
  const anchor = card.id ? `proposal-${card.id}` : undefined;
  const follow = followUp(state);
  const note = followUpNote(follow, frame);
  const error = card.error ?? validation ?? data.error;
  const verb = kind.startsWith("New") ? "Create" : kind.startsWith("Delete") ? "Delete" : "Apply";
  // Busy on a decided card can only be Undo, whichever view clicked it.
  const working = proposal?.status === "created" ? "Undoing…" : acting === "discard" ? "Discarding…" : `${{ Create: "Creating", Delete: "Deleting", Apply: "Applying" }[verb]}…`;
  const decide = (label: string) => frame.failure === "load"
    ? <Button variant="outline" size="sm" onClick={card.retry}><ArrowClockwise />Try again</Button>
    : <><Button variant="ghost" size="sm" disabled={disabled} onClick={() => act("discard")}>{continueLabel("Discard", follow)}</Button><Button variant={kind.startsWith("Delete") ? "destructive" : "default"} size="sm" disabled={disabled || (count === 0 && kind !== "NewProject")} onClick={apply}>{frame.phase === "applying" && acting === "apply" && <SpinnerGap className="animate-spin" />}{continueLabel(label, follow)}</Button></>;

  if (frame.collapsed) {
    const index = version.versions.findIndex((item) => item.id === card.id) + 1;
    const newer = proposal?.superseded_by;
    return <CardFrame id={anchor} state={frame} icon={Icon} note={note} title={title.replace(/\?$/, "")} meta={frame.phase === "discarded" ? <Badge variant="secondary">Discarded</Badge> : <>
      <Badge variant="secondary">{index > 0 ? `V${index} · replaced` : "Replaced"}</Badge>
      {newer && <a className="inline-flex items-center gap-1 text-xs text-brand hover:underline" href={`#proposal-${newer}`} onClick={(event) => { event.preventDefault(); document.getElementById(`proposal-${newer}`)?.scrollIntoView({ behavior: "smooth", block: "center" }); }}>View newer version<ArrowDown size={12} /></a>}
    </>} />;
  }

  const small = kind !== "EditItems" && listing.length === 1 && !listing[0].parent && !["NewProject", "NewNote", "NewCheatNotes", "TagChanges"].includes(kind) && (!listing[0].row.from || Object.keys(object(listing[0].row.to)).length === 1) && !Object.keys(object(listing[0].row.to)).some((field) => ["body", "brief", "notes"].includes(field));
  if (small && !expanded && editor) {
    const heading = <span className="inline-flex max-w-full min-w-0 items-center gap-1.5"><button type="button" disabled={disabled} onClick={() => setExpanded(true)} aria-label="Edit proposal" className="min-w-0 cursor-pointer text-left break-words enabled:hover:text-brand disabled:cursor-default">{title}</button><ChangedSince fields={data.loaded ? conflicts(listing[0].row, data.live[liveKey(listing[0].row)]) : []} live={data.live[String(rowId(listing[0].row))]} /></span>;
    return <CardFrame compact id={anchor} state={frame} icon={Icon} title={heading} error={error} note={note} status={frame.phase === "applying" ? working : undefined}
      meta={editor && <VersionControl inline version={version} disabled={disabled} />}
      actions={decide(verb)} />;
  }

  return <CardFrame id={anchor} state={frame} icon={Icon} error={error} note={note}
    title={editor ? title : resultTitle(proposal!, data.project)}
    subtitle={<>{data.project?.subject != null && <p className="mt-0.5 text-xs text-muted-foreground">{String(data.project.subject)}</p>}{editor && <VersionControl version={version} disabled={disabled} />}</>}
    meta={editor && total > 1 && kind !== "NewProject" && <span className="rounded-full bg-surface px-2 py-0.5 text-xs tabular-nums text-muted-foreground">{count} of {total}</span>}
    status={frame.phase === "applying" ? working : undefined}
    actions={editor ? decide(applyLabel) : proposal?.status === "created" && <Button variant="outline" size="sm" disabled={frame.phase !== "created"} onClick={() => act("undo")}>{frame.phase === "applying" ? <SpinnerGap className="animate-spin" /> : <ArrowCounterClockwise />}Undo</Button>}>
    {editor ? <>
      {kind === "NewProject" && <div className="border-t border-border-subtle px-4">
        <ViewTabs value={tab} onChange={setTab} tabs={[{ value: "attributes", label: "Attributes" }, { value: "tasks", label: "Tasks", badge: <span className="rounded-full bg-surface px-1.5 text-[11px] tabular-nums text-muted-foreground">{count === total ? total : `${count} of ${total}`}</span> }]} />
      </div>}
      {kind === "NewProject" && tab === "attributes" && <ProjectAttributes key={`project:${version.generation}`} state={state} disabled={disabled} onChange={change} />}
      {kind === "NewNote" && <NoteProposal key={`note:${version.generation}`} state={state} disabled={disabled} onChange={change} />}
      {/* Under the tab strip's own rule, the first row drops its top border. */}
      {(kind !== "NewProject" || tab === "tasks") && <div className={cn("contents", kind === "NewProject" && "block [&>*:first-child]:border-t-0")}>{listing.slice(page * 5, page * 5 + 5).map((item, visibleIndex, visible) => {
        const live = data.live[item.row.entity ? liveKey(item.row) : String(rowId(item.row) ?? item.parent?.task)];
        const previousRows = key && card.amended ? viewRows(kind, card.amended, key) : [];
        const previous = previousRows.find((old) => (rowId(item.row) != null && liveKey(old.row) === liveKey(item.row)) || (item.row.title != null && old.row.title === item.row.title))?.row ?? previousRows.find((old) => JSON.stringify(old.path) === JSON.stringify(item.path))?.row;
        const edit = (row: Row) => change(replaceAt(state, item.path, row));
        const editorKey = `${version.generation}:${item.path.join(".")}`;
        if (kind === "NewCheatNotes") return <CheatRow key={editorKey} row={item.row} allowed={item.allowed} previous={card.amended ? previous : undefined} topics={data.topics} disabled={disabled} onChange={edit} />;
        return kind.startsWith("New") ? <CreationRow key={editorKey} kind={kind} item={{ ...item, showParent: visibleIndex === 0 || JSON.stringify(visible[visibleIndex - 1].parentPath) !== JSON.stringify(item.parentPath) }} live={live} previous={card.amended ? previous : undefined} columns={columns(item.parent?.task != null ? live : data.project)} topics={data.topics} disabled={disabled} onChange={edit} /> : <ChangeRow key={editorKey} kind={kind} row={item.row} previous={card.amended ? previous : undefined} live={live} topics={data.topics} loaded={data.loaded} columns={columns(live)} disabled={disabled} onChange={edit} />;
      })}
      <Pager page={page} count={total} onChange={(next) => progress.setValue(next)} /></div>}
    </> : <Results proposal={proposal!} />}
  </CardFrame>;
}
