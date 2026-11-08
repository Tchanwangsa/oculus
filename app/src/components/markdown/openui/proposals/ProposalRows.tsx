import { useState } from "react";
import { ArrowRight, BookOpen, Trash } from "@phosphor-icons/react";
import { DateTimeField } from "@/components/projects/fields/DateTimeField";
import { StatusPill } from "@/components/projects/board/StatusPill";
import type { ProjectColumn } from "@/lib/planning/projects";
import type { RevisionTopic } from "@/lib/revision/types";
import { topicNames } from "./RevisionFields";
import { cn } from "@/lib/utils";
import { TagDot } from "@/components/projects/tags/TagDot";
import { tagColorLabel } from "@/components/projects/tags/tagColors";
import { isTagColor, type TagColor } from "@/lib/planning/projects/tags";
import { Fields, fieldLabel, TextDiff, MARKDOWN_FIELDS } from "./Fields";
import { MarkdownInput } from "./MarkdownInput";
import { CARD_ROW } from "./CardFrame";
import { CHIP, SubjectLabel } from "./SubjectLabel";
import { SubjectPicker } from "./SubjectPicker";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { PILL } from "@/components/projects/fields/pill";
import { Input } from "@/components/ui/input";
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

/** A date in a row's chip line: the same chip as its neighbours, still a picker. */
export const DATE_CHIP = "h-5 rounded-full border-0 bg-surface px-2 py-0 text-[11px] leading-none hover:bg-surface-raised";
/** A changed field's mark against the student's saved card. */
const Updated = ({ label }: { label: string }) => <span className="size-1.5 shrink-0 rounded-full bg-brand" aria-label={label} />;

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
  // The column Create would use: a subtask inherits its parent's, else the board's first.
  const inherited = item.parent ? item.parent.task != null ? live?.column : item.parent.column : null;
  const status = String(row.column ?? inherited ?? columns[0]?.id ?? "backlog");
  return <div className={cn(CARD_ROW, item.parent && "pl-11", item.parent && !item.showParent && "before:left-11")}>
    {item.parent && item.showParent && <div className="mb-1.5 -ml-7 flex min-h-5 flex-wrap items-center gap-1.5 text-xs text-muted-foreground"><span className="min-w-0 break-words">Under <span className="text-foreground">{String(parentTitle)}</span></span>{live?.column != null && <span className={CHIP}>{columns.find((column) => column.id === live.column)?.name as string ?? String(live.column)}</span>}{live?.due != null && <span className={CHIP}>Due {displayValue(live.due, "due")}</span>}</div>}
    <div className="flex min-w-0 items-start gap-3">
      <Include title={title} checked={on} disabled={disabled || !item.allowed} onChange={(checked) => onChange({ ...row, included: checked })} />
      <div className="min-w-0 flex-1">
        <div className="flex min-h-5 items-center gap-1.5">{!events && <fieldset disabled={disabled} className="contents"><StatusPill glyphOnly project={{ columns: columns as unknown as ProjectColumn[] }} columnId={status} onPick={(column) => onChange({ ...row, column })} className="-ml-0.5" /></fieldset>}<button type="button" disabled={disabled} onClick={() => setBefore(before ?? row)} className={cn("min-w-0 cursor-pointer text-left text-[13px] leading-5 break-words enabled:hover:text-brand disabled:cursor-default", !on && "text-muted-foreground line-through")}>{title}</button>{changed?.includes("title") && <Updated label="Updated title" />}</div>
        <div className={cn("mt-1 flex flex-wrap items-center gap-1.5 text-muted-foreground", !events && "pl-6", !on && "opacity-60")}>
          <DateTimeField allowAllDay className={DATE_CHIP} value={typeof row[dateField] === "string" ? row[dateField] as string : null} label={events ? "Start" : "Due"} placeholder={events ? "Set start" : "No due date"} defaultTime={events ? "start" : "end"} onCommit={(value) => { if (!disabled) onChange({ ...row, [dateField]: value }); }} />
          {changed?.includes(dateField) && <Updated label={`Updated ${dateField}`} />}
          {row.estimate != null && <span className={CHIP}><span className="tabular-nums">{String(row.estimate)}</span> min</span>}
          {events && row.kind != null && <span className={CHIP}>{String(row.kind)}</span>}
          {typeof row.subject === "string" && row.subject && <SubjectLabel code={row.subject} />}
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
  const fields = tagCreate ? { color: row.color } : changes;
  const toggle = (checked: boolean) => {
    onChange(includeChange(row, checked, live));
  };
  const lost = row.tag != null && typeof live?.task_count === "number" ? live.task_count ? `${live.task_count} ${live.task_count === 1 ? "task loses" : "tasks lose"} this tag` : "No tasks use this tag" : null;
  return <div className={CARD_ROW}>
    <div className="flex min-w-0 items-start gap-3">
      <Include title={title} checked={on} disabled={disabled || (loaded && !live && !tagCreate)} onChange={toggle} />
      <div className={cn("min-w-0 flex-1", !on && "text-muted-foreground")}>
        <div className="flex min-h-5 items-start gap-1.5">{kind === "TagChanges" && isTagColor(live?.color ?? row.color) && <span className="flex h-5 items-center"><TagDot color={(live?.color ?? row.color) as TagColor} /></span>}<span className={cn("min-w-0 text-[13px] leading-5 break-words", deletion && on && "line-through decoration-destructive/60")}>{title}</span>{tagCreate && <span className={cn(CHIP, "shrink-0")}>New tag</span>}<span className="flex h-5 shrink-0 items-center"><ChangedSince fields={stale} live={live} /></span></div>
        {deletion && <div className={cn("mt-0.5 flex items-center gap-1 text-xs leading-5", on ? "text-destructive" : "text-muted-foreground")}><Trash size={12} className="shrink-0" /><span className="min-w-0">{kind === "DeleteTasks" ? "Deleted with its subtasks" : "Deleted"}{lost ? ` · ${lost}` : ""}</span></div>}
        {deletion && Array.isArray(live?.children) && live.children.length > 0 && <ul className="mt-1 space-y-0.5 border-l border-border-subtle pl-3 text-xs leading-5 text-muted-foreground">{rows(live.children).map((task, i) => <li key={i} className="line-through">{String(task.title)}</li>)}</ul>}
        {Object.keys(fields).length > 0 && <dl className="mt-1 grid grid-cols-[minmax(3.5rem,auto)_minmax(0,1fr)] items-start gap-x-3 gap-y-1">
          {Object.entries(fields).map(([key, value]) => {
            const before = object(row.from)[key], delta = dateShift(before, value);
            const long = ["body", "brief", "notes", "prompt", "answer", "description", "program"].includes(key);
            const text = (value: unknown) => key === "topic_ids" ? topicNames(value, topics) : key === "column" ? columns.find((column) => column.id === value)?.name as string ?? displayValue(value, key) : displayValue(value, key);
            // A tag colour reads as its swatch and name, as in the tag picker.
            const show = (value: unknown) => key === "color" && isTagColor(value) ? <span className="inline-flex items-center gap-1.5"><TagDot color={value} />{tagColorLabel(value)}</span> : text(value);
            const date = ["due", "starts", "start", "end"].includes(key) && !disabled;
            return <div key={key} className="contents">
              <dt className="flex min-h-5 items-center gap-1.5 text-xs leading-5 text-muted-foreground">{fieldLabel(key)}{previous && JSON.stringify(object(previous.to)[key]) !== JSON.stringify(value) && <Updated label={`Updated ${key}`} />}</dt>
              <dd className="flex min-h-5 min-w-0 flex-wrap items-center gap-x-1.5 gap-y-0.5 text-[13px] leading-5">
                {long ? <TextDiff before={before} after={value} markdown={MARKDOWN_FIELDS.has(key)} /> : <>
                  {!tagCreate && <><span className="min-w-0 break-words text-muted-foreground line-through decoration-muted-foreground/50">{show(before)}</span><ArrowRight size={12} className="shrink-0 text-muted-foreground" /></>}
                  {date ? <DateTimeField allowAllDay className="-mx-1.5 h-5 rounded-full border-0 bg-transparent px-1.5 py-0 text-[13px] font-medium hover:bg-surface" value={typeof value === "string" ? value : null} label={`New ${fieldLabel(key)}`} onCommit={(next) => onChange({ ...row, to: { ...object(row.to), [key]: next } })} /> : <span className={cn("min-w-0 break-words", on && "font-medium text-foreground")}>{show(value)}</span>}
                  {delta && <span className={CHIP}><span className="tabular-nums">{delta}</span></span>}
                </>}
              </dd>
            </div>;
          })}
        </dl>}
        {editing && <fieldset disabled={disabled}><Fields row={tagCreate ? row : object(row.to)} names={tagCreate ? ["name", "color"] : Object.keys(fields)} columns={columns} topics={topics} subjectId={"subject_id" in object(row.to) ? typeof object(row.to).subject_id === "number" ? Number(object(row.to).subject_id) : null : typeof live?.subject_id === "number" ? live.subject_id : null} entity={String(row.entity ?? "")} onChange={(next) => onChange(tagCreate ? next : { ...row, to: next })} /></fieldset>}
      </div>
      {!deletion && <RowActions title={title} editing={editing} disabled={disabled} canRemove={on} onEdit={() => setBefore(row)} onSave={() => setBefore(null)} onCancel={() => { onChange(before!); setBefore(null); }} onRemove={() => toggle(false)} />}
    </div>
  </div>;
}

/** A proposed note, laid out as a project's attributes are: the subject as a
 * property pill, then the title and the body. */
export function NoteProposal({ state, disabled, onChange }: { state: Row; disabled: boolean; onChange: (row: Row) => void }) {
  const subject = typeof state.subject === "string" && state.subject ? state.subject : null;
  return <fieldset disabled={disabled} className="flex min-w-0 flex-col gap-3 px-4 py-3">
    <div className="flex flex-wrap items-center gap-2" aria-label="Note properties">
      <SubjectPicker value={subject} triggerClassName={PILL} icon={subject ? <SubjectIcon code={subject} size={13} /> : <BookOpen size={13} />} onChange={(code) => onChange({ ...state, subject: code })} />
    </div>
    <label className="flex min-w-0 flex-col gap-1 text-xs"><span className="text-muted-foreground">Title</span><Input aria-label="Title" value={String(state.title ?? "")} onChange={(e) => onChange({ ...state, title: e.target.value })} className="h-8 text-[13px]" /></label>
    <div className="flex min-w-0 flex-col gap-1 text-xs"><span className="text-muted-foreground">Body</span><MarkdownInput value={state.body} label="Body" subject={state.subject} onChange={(body) => onChange({ ...state, body })} /></div>
  </fieldset>;
}
