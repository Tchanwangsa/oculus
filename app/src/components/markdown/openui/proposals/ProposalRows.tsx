import { useState } from "react";
import { ArrowRight } from "@phosphor-icons/react";
import { DateTimeField } from "@/components/projects/fields/DateTimeField";
import type { RevisionTopic } from "@/lib/revision/types";
import { topicNames } from "./RevisionFields";
import { cn } from "@/lib/utils";
import { Fields, fieldLabel, TextDiff, Field } from "./Fields";
import { ChangedSince, dateShift, displayValue, Include, RowActions } from "./ProposalParts";
import { conflicts, effectiveChanges, included, includeChange, object, rowId, rows, type Row } from "./model";

export interface ViewRow { row: Row; path: (string | number)[]; parent?: Row; parentPath?: (string | number)[]; allowed: boolean }

export function viewRows(kind: string, state: Row, key: string): ViewRow[] {
  return rows(state[key]).flatMap((row, i) => {
    const path = [key, i];
    const children = rows(row.subtasks).map((child, j) => ({ row: child, path: [...path, "subtasks", j], parent: row, parentPath: path, allowed: included(row) }));
    if (kind === "NewTasks" && row.task != null) return children;
    return [{ row, path, allowed: true }, ...children];
  });
}

export function replaceAt(root: Row, path: (string | number)[], value: Row): Row {
  if (!path.length) return value;
  const [key, ...tail] = path;
  if (Array.isArray(root)) {
    const next = [...root];
    next[Number(key)] = replaceAt(root[Number(key)] as Row, tail, value);
    return next as unknown as Row;
  }
  return { ...root, [key]: replaceAt(root[key] as Row, tail, value) };
}

export function CreationRow({ kind, item, live, previous, columns, topics, disabled, onChange }: {
  kind: string; item: ViewRow & { showParent?: boolean }; live?: Row | null; previous?: Row; columns: Row[]; topics?: RevisionTopic[]; disabled: boolean; onChange: (row: Row) => void;
}) {
  const [before, setBefore] = useState<Row | null>(null);
  const editing = before !== null;
  const row = item.row;
  const title = String(row.title ?? row.name ?? "Untitled");
  const on = item.allowed && included(row);
  const parentTitle = item.parent?.task != null ? live?.title ?? `Task ${item.parent.task}` : item.parent?.title;
  const events = kind === "NewEvents";
  const dateField = events ? "start" : "due";
  const changed = previous && Object.keys(row).filter((key) => key !== "included" && JSON.stringify(row[key]) !== JSON.stringify(previous[key]));
  return <div className={cn("border-t border-border-subtle px-4 py-2.5", item.parent && "pl-7")}>
    {item.parent && item.showParent && <div className="mb-1 flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground"><span>Under {String(parentTitle)}</span>{live?.column != null && <span className="rounded-full bg-surface px-2 py-0.5">{columns.find((column) => column.id === live.column)?.name as string ?? String(live.column)}</span>}{live?.due != null && <span>Due {displayValue(live.due, "due")}</span>}</div>}
    <div className="flex min-w-0 items-start gap-3">
      <Include title={title} checked={on} disabled={disabled || !item.allowed} onChange={(checked) => onChange({ ...row, included: checked })} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5"><button type="button" disabled={disabled} onClick={() => setBefore(before ?? row)} className={cn("min-w-0 cursor-pointer text-left text-[13px] break-words hover:text-brand", !on && "text-muted-foreground line-through")}>{title}</button>{changed?.includes("title") && <span className="size-1.5 shrink-0 rounded-full bg-brand" aria-label="Updated title" />}</div>
        <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
          <DateTimeField allowAllDay value={typeof row[dateField] === "string" ? row[dateField] as string : null} label={events ? "Start" : "Due"} placeholder={events ? "Set start" : "No due date"} defaultTime={events ? "start" : "end"} onCommit={(value) => { if (!disabled) onChange({ ...row, [dateField]: value }); }} />
          {changed?.includes(dateField) && <span className="size-1.5 rounded-full bg-brand" aria-label={`Updated ${dateField}`} />}
          {row.estimate != null && <span className="rounded-full bg-surface px-2 py-0.5">{String(row.estimate)} min</span>}
          {row.column != null && <span className="rounded-full bg-surface px-2 py-0.5">{columns.find((column) => column.id === row.column)?.name as string ?? String(row.column)}</span>}
          {events && row.kind != null && <span>{String(row.kind)}</span>}
          {row.subject != null && <span>{String(row.subject)}</span>}
        </div>
      </div>
      <RowActions title={title} editing={editing} disabled={disabled} canRemove={on} onEdit={() => setBefore(row)} onSave={() => setBefore(null)} onCancel={() => { onChange(before!); setBefore(null); }} onRemove={() => onChange({ ...row, included: false })} />
    </div>
    {editing && <fieldset disabled={disabled} className="mt-1 ml-7 min-w-0"><Fields row={row} topics={topics} columns={columns} names={events ? ["title", "start", "end", "kind", "subject", "notes"] : ["title", "due", "estimate", "column", "starts", "tags", "body", "files"]} onChange={onChange} /></fieldset>}
  </div>;
}

export function ChangeRow({ kind, row, previous, live, loaded, disabled, columns, topics = [], onChange }: {
  kind: string; row: Row; previous?: Row; live: Row | null | undefined; loaded: boolean; disabled: boolean; columns: Row[]; topics?: RevisionTopic[]; onChange: (row: Row) => void;
}) {
  const [before, setBefore] = useState<Row | null>(null);
  const editing = before !== null;
  const stale = loaded ? conflicts(row, live) : [];
  const on = included(row) && (row.included !== undefined || !stale.length);
  const title = String(live?.title ?? live?.name ?? live?.prompt ?? row.name ?? `${kind.replace("Changes", "")} ${rowId(row) ?? ""}`);
  const changes = effectiveChanges(row);
  const tagCreate = kind === "TagChanges" && row.name != null && !row.from;
  const deletion = kind.startsWith("Delete") || (kind === "TagChanges" && row.tag != null);
  const fields = tagCreate ? { name: row.name, color: row.color } : changes;
  const toggle = (checked: boolean) => {
    onChange(includeChange(row, checked, live));
  };
  return <div className="border-t border-border-subtle px-4 py-2.5">
    <div className="flex min-w-0 items-start gap-3">
      <Include title={title} checked={on} disabled={disabled || (loaded && !live && !tagCreate)} onChange={toggle} />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-1.5"><span className={cn("text-[13px] break-words", !on && "text-muted-foreground")}>{title}</span><ChangedSince fields={stale} live={live} /></div>
        {deletion && <p className="mt-1 text-xs text-destructive">Delete{kind === "DeleteTasks" ? " with its subtasks" : ""}{row.tag != null && live?.task_count != null ? ` · ${live.task_count} tasks lose this tag` : ""}</p>}
        {deletion && Array.isArray(live?.children) && live.children.length > 0 && <ul className="mt-1 pl-3 text-xs text-muted-foreground">{rows(live.children).map((task, i) => <li key={i}>{String(task.title)}</li>)}</ul>}
        {Object.entries(fields).map(([key, value]) => {
          const before = object(row.from)[key], delta = dateShift(before, value);
          const long = ["body", "brief", "notes", "prompt", "answer", "description", "program"].includes(key);
          const show = (value: unknown) => key === "topic_ids" ? topicNames(value, topics) : displayValue(value, key);
          return <div key={key} className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px]">
            <span className="text-muted-foreground">{fieldLabel(key)}</span>
            {previous && JSON.stringify(object(previous.to)[key]) !== JSON.stringify(value) && <span className="size-1.5 rounded-full bg-brand" aria-label={`Updated ${key}`} />}
            {long ? <div className="w-full"><TextDiff before={before} after={value} /></div> : <>
              {!tagCreate && <><span className="rounded-full bg-surface px-2 py-0.5 text-muted-foreground line-through">{key === "column" ? columns.find((column) => column.id === before)?.name as string ?? show(before) : show(before)}</span><ArrowRight size={12} className="text-muted-foreground" /></>}
              {["due", "starts", "start", "end"].includes(key) && !disabled ? <DateTimeField allowAllDay value={typeof value === "string" ? value : null} label={`New ${fieldLabel(key)}`} onCommit={(next) => onChange({ ...row, to: { ...object(row.to), [key]: next } })} /> : <span className="max-w-full rounded-full bg-surface px-2 py-0.5 break-words">{key === "column" ? columns.find((column) => column.id === value)?.name as string ?? show(value) : show(value)}</span>}
              {delta && <span className="text-xs text-warning">{delta}</span>}
            </>}
          </div>;
        })}
        {editing && <fieldset disabled={disabled}><Fields row={tagCreate ? row : object(row.to)} names={Object.keys(fields)} columns={columns} topics={topics} subjectId={"subject_id" in object(row.to) ? typeof object(row.to).subject_id === "number" ? Number(object(row.to).subject_id) : null : typeof live?.subject_id === "number" ? live.subject_id : null} entity={String(row.entity ?? "")} onChange={(next) => onChange(tagCreate ? next : { ...row, to: next })} /></fieldset>}
      </div>
      {!deletion && <RowActions title={title} editing={editing} disabled={disabled} canRemove={on} onEdit={() => setBefore(row)} onSave={() => setBefore(null)} onCancel={() => { onChange(before!); setBefore(null); }} onRemove={() => toggle(false)} />}
    </div>
  </div>;
}

export function NoteProposal({ state, disabled, onChange }: { state: Row; disabled: boolean; onChange: (row: Row) => void }) {
  return <fieldset disabled={disabled} className="border-t border-border-subtle px-4 py-2"><Field name="subject" value={state.subject} onChange={(subject) => onChange({ ...state, subject })} /><Fields row={state} names={["title", "body"]} onChange={onChange} /></fieldset>;
}
