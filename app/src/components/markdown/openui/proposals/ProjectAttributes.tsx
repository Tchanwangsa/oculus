import { CalendarBlank, Flag, User } from "@phosphor-icons/react";
import { Input } from "@/components/ui/input";
import { DateTimeField } from "@/components/projects/fields/DateTimeField";
import { PILL } from "@/components/projects/fields/pill";
import { NameTags } from "@/components/projects/tags/NameTags";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useTaskTags } from "@/hooks/data/useTaskTags";
import { Field } from "./Fields";
import { SubjectPicker } from "./SubjectPicker";
import type { Row } from "./model";

const text = (value: unknown) => typeof value === "string" && value ? value : null;

/**
 * A proposed project's own fields, laid out as the project page lays them
 * out: the name, then one row of property pills (subject, due, start, tags),
 * then the calendar event and the brief.
 */
export function ProjectAttributes({ state, disabled, onChange }: { state: Row; disabled: boolean; onChange: (row: Row) => void }) {
  const { tags } = useTaskTags();
  const set = (field: string, value: unknown) => onChange({ ...state, [field]: value });
  const subject = text(state.subject);
  return <fieldset disabled={disabled} className="flex min-w-0 flex-col gap-3 px-4 py-3">
    <label className="flex min-w-0 flex-col gap-1 text-xs">
      <span className="text-muted-foreground">Name</span>
      <Input aria-label="Project name" value={String(state.name ?? "")} onChange={(e) => set("name", e.target.value)} className="h-8 text-[13px]" />
    </label>
    <div className="flex flex-wrap items-center gap-2" aria-label="Project properties">
      <SubjectPicker value={subject} none="Personal" triggerClassName={PILL} icon={subject ? <SubjectIcon code={subject} size={13} /> : <User size={13} />} onChange={(code) => set("subject", code)} />
      <DateTimeField variant="pill" icon={Flag} label="Due date" placeholder="Due date" allowAllDay value={text(state.due)} defaultTime="end" onCommit={(value) => set("due", value)} />
      <DateTimeField variant="pill" icon={CalendarBlank} label="Start date" placeholder="Start" allowAllDay value={text(state.starts)} defaultTime="start" onCommit={(value) => set("starts", value)} />
      <NameTags size="pill" registry={tags} value={Array.isArray(state.tags) ? state.tags.filter((tag): tag is string => typeof tag === "string") : []} disabled={disabled} onChange={(names) => set("tags", names)} />
    </div>
    <Field name="event" value={state.event} subject={state.subject} onChange={(value) => set("event", value)} />
    <Field name="brief" value={state.brief} onChange={(value) => set("brief", value)} />
  </fieldset>;
}
