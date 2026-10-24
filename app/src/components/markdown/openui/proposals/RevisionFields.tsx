import { useState } from "react";
import { FilterPopover } from "@/components/ui/filter/FilterPopover";
import { FilterChecklist } from "@/components/ui/filter/FilterChecklist";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { useSubjects } from "@/hooks/data/useSubjects";
import type { RevisionTopic } from "@/lib/revision/types";

export function TopicsField({ value, topics, subjectId, subject, onChange }: {
  value: unknown; topics: RevisionTopic[]; subjectId?: number | null; subject?: unknown; onChange: (value: unknown) => void;
}) {
  const { subjects } = useSubjects();
  const scope = subjectId ?? (typeof subject === "string" ? subjects.find((row) => row.code === subject)?.id : null) ?? null;
  const choices = topics.filter((topic) => topic.subject_id === scope && !topic.is_unsorted);
  const selected = new Set(Array.isArray(value) ? value.map(String) : []);
  const names = choices.filter((topic) => selected.has(String(topic.id))).map((topic) => topic.name);
  return <FilterPopover label={names.join(", ") || "Unsorted"} active={names.length > 0}>{() => <FilterChecklist noun="topics" options={choices.map((topic) => ({ value: String(topic.id), label: topic.name }))} selected={selected} onChange={(ids) => onChange([...ids].map(Number))} />}</FilterPopover>;
}

/**
 * Topics by name for a new entry: the subject's topics to tick, the card's
 * new names among them, and a field to type another. Add creates new names
 * in the entry's subject.
 */
export function TopicNamesField({ value, topics, subject, triggerClassName, onChange }: {
  value: unknown; topics: RevisionTopic[]; subject?: unknown; triggerClassName?: string; onChange: (value: unknown) => void;
}) {
  const { subjects } = useSubjects();
  const [draft, setDraft] = useState("");
  const scope = typeof subject === "string" ? subjects.find((row) => row.code === subject)?.id : undefined;
  const chosen = Array.isArray(value) ? value.filter((v): v is string => typeof v === "string") : [];
  const existing = topics.filter((topic) => topic.subject_id === scope && !topic.is_unsorted).map((topic) => topic.name);
  const known = (name: string) => existing.some((e) => e.toLowerCase() === name.toLowerCase());
  const names = [...existing, ...chosen.filter((name) => !known(name))];
  const selected = new Set(names.filter((name) => chosen.some((c) => c.toLowerCase() === name.toLowerCase())));
  const add = () => {
    const name = draft.trim();
    if (name && !chosen.some((c) => c.toLowerCase() === name.toLowerCase())) onChange([...chosen, name]);
    setDraft("");
  };
  return <FilterPopover label={chosen.join(", ") || "Unsorted"} active={chosen.length > 0} triggerClassName={triggerClassName}>{() => <>
    <FilterChecklist noun="topics" options={names.map((name) => ({ value: name, label: known(name) ? name : `${name} (new)` }))} selected={selected} onChange={(next) => onChange([...next])} />
    <div className="border-t border-border-subtle p-1.5"><Input aria-label="New topic" placeholder="New topic" value={draft} onChange={(e) => setDraft(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); add(); } }} className="h-7 text-[12.5px]" /></div>
  </>}</FilterPopover>;
}

export function JsonField({ label, value, onChange }: { label: string; value: unknown; onChange: (value: unknown) => void }) {
  const [text, setText] = useState(JSON.stringify(value, null, 2));
  const [error, setError] = useState<string>();
  return <div><Textarea aria-label={label} value={text} onChange={(event) => {
    const next = event.target.value;
    setText(next);
    try { onChange(JSON.parse(next)); setError(undefined); }
    catch { onChange(next); setError("Enter valid JSON before applying this edit."); }
  }} />{error && <p role="alert" className="text-xs text-destructive">{error}</p>}</div>;
}

export function topicNames(value: unknown, topics: RevisionTopic[]) {
  return Array.isArray(value) && value.length ? value.map((id) => topics.find((topic) => topic.id === id)?.name ?? `Topic ${id}`).join(", ") : "Unsorted";
}
