import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { DateTimeField } from "@/components/projects/fields/DateTimeField";
import { ProjectPicker } from "@/components/projects/nav/ProjectPicker";
import { getProjects } from "@/lib/planning/projects/reads";
import type { DbProject } from "@/lib/planning/projects/rows";
import { getSubjects, type Subject } from "@/lib/db";
import { Button } from "@/components/ui/button";
import { displayValue } from "./ProposalParts";
import { object, type Row } from "./model";

import { TopicsField, TopicNamesField, JsonField } from "./RevisionFields";
import type { RevisionTopic } from "@/lib/revision/types";
const LONG = new Set(["body", "brief", "notes", "prompt", "answer", "description", "program"]);
const EventLink = lazy(() => import("@/components/projects/page/EventLink").then((module) => ({ default: module.EventLink })));
const DATES = new Set(["due", "starts", "start", "end"]);
const LISTS = new Set(["tags", "files", "topic"]);
const LABELS: Record<string, string> = { topic_ids: "Topics", topics: "Topics", subject_id: "Subject id", due: "Due", starts: "Starts", estimate: "Estimate (minutes)", column: "Column", body: "Description", files: "Related files", event: "Calendar event" };
export const fieldLabel = (key: string) => LABELS[key] ?? key[0].toUpperCase() + key.slice(1);

function ListField({ name, value, onChange }: { name: string; value: unknown; onChange: (value: unknown) => void }) {
  const separator = name === "files" ? "\n" : ", ";
  const source = Array.isArray(value) ? value.join(separator) : "";
  const [raw, setRaw] = useState(source);
  const editing = useRef(false);
  useEffect(() => { if (!editing.current) setRaw(source); }, [source]);
  const change = (text: string) => {
    setRaw(text);
    onChange(text.split(name === "files" ? "\n" : ",").map((part) => part.trim()).filter(Boolean));
  };
  const props = { "aria-label": fieldLabel(name), value: raw, onFocus: () => { editing.current = true; }, onBlur: () => { editing.current = false; setRaw(source); }, onChange: (event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => change(event.target.value) };
  return name === "files" ? <Textarea {...props} className="min-h-20 text-[13px]" /> : <Input {...props} className="h-8 text-[13px]" />;
}

export function Options({ value, options, label, onChange }: { value: unknown; options: { id: string; label: string }[]; label: string; onChange: (value: string) => void }) {
  return <Select value={typeof value === "string" && value ? value : "__empty"} onValueChange={(next) => onChange(next === "__empty" ? "" : next)}>
    <SelectTrigger size="sm" aria-label={label} className="max-w-full"><SelectValue placeholder={label} /></SelectTrigger>
    <SelectContent><SelectItem value="__empty">None</SelectItem>{options.map((option) => <SelectItem key={option.id} value={option.id}>{option.label}</SelectItem>)}</SelectContent>
  </Select>;
}

function ProjectField({ value, onChange }: { value: unknown; onChange: (value: unknown) => void }) {
  const [projects, setProjects] = useState<DbProject[]>([]);
  const [error, setError] = useState<string>();
  useEffect(() => { let active = true; void getProjects({ status: "all" }).then((next) => { if (active) setProjects(next); }).catch((e) => { if (active) setError(String(e)); }); return () => { active = false; }; }, []);
  return <div><ProjectPicker projects={projects} value={typeof value === "number" ? value : null} onPick={onChange}><Button variant="outline" size="sm">{projects.find((p) => p.id === value)?.name ?? (value == null ? "No project" : `Project ${value}`)}</Button></ProjectPicker>{error && <p className="text-xs text-destructive">{error}</p>}</div>;
}

function SubjectField({ value, onChange }: { value: unknown; onChange: (value: unknown) => void }) {
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [error, setError] = useState<string>();
  useEffect(() => { let active = true; void getSubjects().then((next) => { if (active) setSubjects(next); }).catch((e) => { if (active) setError(String(e)); }); return () => { active = false; }; }, []);
  const options = subjects.map((subject) => ({ id: subject.code, label: subject.code }));
  if (typeof value === "string" && value && !options.some((option) => option.id === value)) options.unshift({ id: value, label: value });
  return <div><Options value={value} options={options} label="Subject" onChange={(next) => onChange(next || null)} />{error && <p className="text-xs text-destructive">{error}</p>}</div>;
}

function EventField({ value, subject, onChange }: { value: unknown; subject: unknown; onChange: (value: unknown) => void }) {
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [error, setError] = useState<string>();
  useEffect(() => { let active = true; void getSubjects().then((next) => { if (active) setSubjects(next); }).catch((e) => { if (active) setError(String(e)); }); return () => { active = false; }; }, []);
  const match = typeof subject === "string" ? subjects.find((row) => row.code === subject) : null;
  return <div><Suspense fallback={<span className="text-xs text-muted-foreground">Loading calendar…</span>}><EventLink project={{ event_id: typeof value === "string" ? value : null, subject_id: match?.id ?? null }} onPick={onChange} /></Suspense>{error && <p className="text-xs text-destructive">{error}</p>}</div>;
}

export function Field({ name, value, onChange, columns, subject, topics = [], subjectId, entity }: { name: string; value: unknown; onChange: (value: unknown) => void; columns?: Row[]; subject?: unknown; topics?: RevisionTopic[]; subjectId?: number | null; entity?: string }) {
  const label = fieldLabel(name);
  let control;
  if (name === "topic_ids") control = <TopicsField value={value} topics={topics} subjectId={subjectId} subject={subject} onChange={onChange} />;
  else if (name === "topics") control = <TopicNamesField value={value} topics={topics} subject={subject} onChange={onChange} />;
  else if (["options", "rubric", "state"].includes(name)) control = <JsonField label={label} value={value} onChange={onChange} />;
  else if (["subject_id", "position", "tolerance"].includes(name)) control = <Input aria-label={label} type="number" value={value == null ? "" : String(value)} onChange={(e) => onChange(e.target.value === "" ? null : Number(e.target.value))} />;
  else if (entity === "question" && ["kind", "status", "level"].includes(name)) {
    const options = name === "kind" ? ["card", "choice", "numeric", "short", "structured"] : name === "status" ? ["draft", "active", "suspended"] : ["easy", "medium", "hard"];
    control = <Options value={value} options={options.map((id) => ({ id, label: id }))} label={label} onChange={onChange} />;
  }
  else if (DATES.has(name)) control = <DateTimeField allowAllDay value={typeof value === "string" ? value : null} onCommit={onChange} label={label} defaultTime={name === "starts" || name === "start" ? "start" : "end"} />;
  else if (name === "project") control = <ProjectField value={value} onChange={onChange} />;
  else if (name === "subject") control = <SubjectField value={value} onChange={onChange} />;
  else if (name === "event") control = <EventField value={value} subject={subject} onChange={onChange} />;
  else if (name === "column" && columns?.length) control = <Options value={value} options={columns.map((c) => ({ id: String(c.id), label: String(c.name) }))} label={label} onChange={(next) => onChange(next || null)} />;
  else if (name === "kind" || name === "status" || name === "color") {
    const options = name === "kind" ? ["note", "due", "class"] : name === "status" ? ["active", "archived"] : ["slate", "indigo", "green", "amber", "rose", "purple", "teal", "orange"];
    control = <Options value={value} options={options.map((id) => ({ id, label: id }))} label={label} onChange={(next) => onChange(next || null)} />;
  } else if (LISTS.has(name)) control = <ListField name={name} value={value} onChange={onChange} />;
  else if (LONG.has(name)) control = <Textarea aria-label={label} className="min-h-20 text-[13px]" value={String(value ?? "")} onChange={(e) => onChange(e.target.value)} />;
  else control = <Input aria-label={label} className="h-8 text-[13px]" type={name === "estimate" ? "number" : "text"} min={name === "estimate" ? 0 : undefined} step={name === "estimate" ? 1 : undefined} value={Array.isArray(value) ? value.join(", ") : String(value ?? "")} onChange={(e) => onChange(LISTS.has(name) ? e.target.value.split(",").map((v) => v.trim()).filter(Boolean) : name === "estimate" ? (e.target.value === "" ? null : Number(e.target.value)) : e.target.value)} />;
  return <label className={`flex min-w-0 flex-col gap-1 text-xs ${LONG.has(name) || name === "files" ? "col-span-full" : ""}`}><span className="text-muted-foreground">{label}</span>{control}</label>;
}

export function Fields({ row, names, onChange, columns, topics, subjectId, entity }: { row: Row; names: string[]; onChange: (row: Row) => void; columns?: Row[]; topics?: RevisionTopic[]; subjectId?: number | null; entity?: string }) {
  return <div className="grid min-w-0 grid-cols-1 gap-3 py-2 @sm/proposal:grid-cols-2">{names.map((name) => <Field key={name} name={name} value={row[name]} subject={row.subject} columns={columns} topics={topics} subjectId={subjectId} entity={entity} onChange={(value) => onChange({ ...row, [name]: value })} />)}</div>;
}

export function TextDiff({ before, after }: { before: unknown; after: unknown }) {
  const [expanded, setExpanded] = useState(false);
  const oldText = String(before ?? ""), newText = String(after ?? "");
  const lines = [...oldText.split("\n").filter((line) => line && !newText.split("\n").includes(line)).map((line) => ({ line, added: false })), ...newText.split("\n").filter((line) => line && !oldText.split("\n").includes(line)).map((line) => ({ line, added: true }))];
  return <div className="min-w-0 text-[13px]">
    {(expanded ? lines : lines.slice(0, 3)).map(({ line, added }, i) => <p key={i} className={`break-words ${added ? "text-success" : "text-muted-foreground line-through"}`}>{added ? "+ " : "− "}{line}</p>)}
    {lines.length > 3 && <Button variant="link" size="xs" onClick={() => setExpanded(!expanded)}>{expanded ? "Show less" : "Show full change"}</Button>}
  </div>;
}

export function ColumnsOf({ row, children }: { row: Row; children: (columns: Row[]) => React.ReactNode }) {
  let columns: unknown = row.columns;
  if (typeof columns === "string") { try { columns = JSON.parse(columns); } catch { columns = []; } }
  return children(Array.isArray(columns) ? columns.map(object) : []);
}

export const valueLabel = displayValue;
