import { useEffect, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { DateTimeField } from "@/components/projects/DateTimeField";
import { createLocalEvent, getSubjects, updateLocalEvent } from "@/lib/db";
import { displayCode } from "@/lib/format";
import {
  CALENDAR_UPDATED_EVENT,
  NO_SUBJECT,
  NO_SUBJECT_LABEL,
} from "@/lib/calendar";
import { useEventEditor } from "@/stores/eventEditorStore";

/**
 * The three layers a row the user writes can join, in the order they are
 * offered. A note is first because it is what most new rows are: a reminder
 * with no Canvas counterpart at all. `class` is the only one with a span —
 * a note and a due date are instants, which is why the end field appears and
 * disappears with this control rather than sitting there greyed.
 */
const KINDS: { id: string; label: string; hint: string }[] = [
  { id: "note", label: "Note", hint: "A reminder pinned to a time." },
  { id: "class", label: "Class", hint: "Something that occupies a stretch of the day." },
  { id: "due", label: "Due", hint: "A cutoff, drawn like a Canvas deadline." },
];

/** What the subject Select calls "no subject". Radix needs a non-empty string
 *  value, and `null` is not one. */
const PERSONAL = "personal";

const HOUR_MS = 60 * 60 * 1000;

/**
 * When a new event starts.
 *
 * On today, the next whole hour — you are almost always adding something ahead
 * of now, and 9am on a day that is half over is a date you would have to fix.
 * On any other day, 9am: the start of the teaching day, and the same default
 * `DateTimeField` uses for a start.
 */
function defaultStart(day: Date | null): Date {
  const now = new Date();
  const base = day ?? now;
  const out = new Date(base);
  const sameDay =
    base.getFullYear() === now.getFullYear() &&
    base.getMonth() === now.getMonth() &&
    base.getDate() === now.getDate();
  if (sameDay) out.setHours(now.getHours() + 1, 0, 0, 0);
  else out.setHours(9, 0, 0, 0);
  return out;
}

/**
 * Write a row into `local_events` — the one calendar layer Oculus owns.
 *
 * It is a dialog rather than an inline row because an event is six fields, two
 * of which are dates; and it is mounted in `AppLayout` because the Edit that
 * opens it lives on `EventPopover`, which is rendered by all three calendar
 * views and by Home's Today list. See `app/src/stores/eventEditorStore.ts`.
 */
export function EventDialog() {
  const open = useEventEditor((s) => s.open);
  const editing = useEventEditor((s) => s.editing);
  const day = useEventEditor((s) => s.day);
  const close = useEventEditor((s) => s.close);
  const [subjects, setSubjects] = useState<{ id: number; label: string }[]>([]);

  const [title, setTitle] = useState("");
  const [kind, setKind] = useState("note");
  const [subject, setSubject] = useState(PERSONAL);
  const [startAt, setStartAt] = useState<string | null>(null);
  const [endAt, setEndAt] = useState<string | null>(null);
  const [allDay, setAllDay] = useState(false);
  const [notes, setNotes] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Seeded when the dialog opens, not on every render: the store hands over a
  // `CalEvent` to edit or a day to start on, and everything typed after that
  // belongs to the form. The subject list is read here too rather than on
  // mount — the dialog is mounted in `AppLayout` for the whole session, and a
  // query at startup for a form nobody has opened is a query for nothing.
  useEffect(() => {
    if (!open) return;
    void getSubjects()
      .then((rows) =>
        setSubjects(rows.map((r) => ({ id: r.id, label: displayCode(r.code) }))),
      )
      .catch(console.error);
    setError(null);
    setSaving(false);
    if (editing) {
      setTitle(editing.title);
      setKind(editing.kind);
      setSubject(
        editing.subjectId === NO_SUBJECT ? PERSONAL : String(editing.subjectId),
      );
      setStartAt(editing.start.toISOString());
      setEndAt(editing.end ? editing.end.toISOString() : null);
      setAllDay(editing.allDay);
      // A local row's `notes` column is what the card renders as its body.
      setNotes(editing.description ?? "");
    } else {
      setTitle("");
      setKind("note");
      setSubject(PERSONAL);
      setStartAt(defaultStart(day).toISOString());
      setEndAt(null);
      setAllDay(false);
      setNotes("");
    }
  }, [open, editing, day]);

  // Only a class occupies time; a note and a due date are instants, and an
  // all-day row of any kind has no clock to end on either.
  const hasEnd = kind === "class" && !allDay;

  /** Switching to Class with no end yet gets an hour, which is what a class
   *  almost always is — rather than an empty field the user has to discover. */
  const pickKind = (next: string) => {
    setKind(next);
    if (next === "class" && !endAt && startAt) {
      setEndAt(new Date(new Date(startAt).getTime() + HOUR_MS).toISOString());
    }
  };

  const save = () => {
    const name = title.trim();
    if (!name) {
      setError("Give the event a title.");
      return;
    }
    if (!startAt) {
      setError("Pick a start.");
      return;
    }
    const end = hasEnd ? endAt : null;
    if (end && new Date(end).getTime() <= new Date(startAt).getTime()) {
      setError("It ends before it starts.");
      return;
    }
    setSaving(true);
    setError(null);
    const input = {
      subjectId: subject === PERSONAL ? null : Number(subject),
      kind,
      title: name,
      startAt,
      endAt: end,
      allDay,
      notes: notes.trim() || null,
    };
    const write =
      editing?.localId != null
        ? updateLocalEvent(editing.localId, input)
        : createLocalEvent(input);
    void write
      .then(() => {
        // The same signal a sync raises, so every open calendar re-reads.
        window.dispatchEvent(new CustomEvent(CALENDAR_UPDATED_EVENT));
        close();
      })
      .catch((e) => {
        console.error(e);
        setError(String(e));
        setSaving(false);
      });
  };

  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{editing ? "Edit event" : "New event"}</DialogTitle>
          <DialogDescription>
            Yours alone — it lives beside the Canvas rows and no sync ever
            touches it.
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-[68px_minmax(0,1fr)] items-center gap-x-3 gap-y-2.5">
          <Field label="Title">
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="What is it?"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === "Enter") save();
              }}
            />
          </Field>

          <Field label="Kind">
            <div className="flex min-w-0 items-center gap-2">
              <ToggleGroup
                type="single"
                size="sm"
                value={kind}
                onValueChange={(v) => v && pickKind(v)}
                className="shrink-0 rounded-md bg-surface p-0.5"
              >
                {KINDS.map((k) => (
                  <ToggleGroupItem
                    key={k.id}
                    value={k.id}
                    title={k.hint}
                    className="h-6 rounded-[5px] px-2.5 text-xs data-[state=on]:bg-card data-[state=on]:shadow-xs"
                  >
                    {k.label}
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
              <span className="flex-1" />
              <label className="flex shrink-0 cursor-pointer items-center gap-1.5 text-[11.5px] text-muted-foreground">
                <Switch checked={allDay} onCheckedChange={setAllDay} />
                All day
              </label>
            </div>
          </Field>

          <Field label="Subject">
            <Select value={subject} onValueChange={setSubject}>
              <SelectTrigger size="sm" className="w-full text-[13px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={PERSONAL}>{NO_SUBJECT_LABEL}</SelectItem>
                {/* Current subjects first, which is the order `getSubjects`
                    already returns. */}
                {subjects.map((o) => (
                  <SelectItem key={o.id} value={String(o.id)}>
                    {o.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>

          <Field label={hasEnd ? "Starts" : "When"}>
            <DateTimeField
              value={startAt}
              onCommit={setStartAt}
              defaultTime="start"
              placeholder="Pick a time"
            />
          </Field>

          {hasEnd && (
            <Field label="Ends">
              <DateTimeField
                value={endAt}
                onCommit={setEndAt}
                defaultTime="end"
                placeholder="Pick a time"
              />
            </Field>
          )}

          <Field label="Notes" align="start">
            <Textarea
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Anything worth keeping with it. Markdown works."
              className="min-h-16 resize-y"
            />
          </Field>
        </div>

        {error && <p className="text-[11.5px] text-destructive">{error}</p>}

        <DialogFooter>
          <Button variant="outline" onClick={close}>
            Cancel
          </Button>
          <Button onClick={save} disabled={saving}>
            {editing ? "Save" : "Add event"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** One labelled row of the form. A plain `<span>` rather than `Label`: the
 *  controls here are a toggle group, a Radix select and two popover buttons,
 *  none of which a `for=` would reach. */
function Field({
  label,
  align = "center",
  children,
}: {
  label: string;
  align?: "center" | "start";
  children: React.ReactNode;
}) {
  return (
    <>
      <span
        className={
          align === "start"
            ? "self-start pt-2 text-[11.5px] text-muted-foreground"
            : "text-[11.5px] text-muted-foreground"
        }
      >
        {label}
      </span>
      <div className="min-w-0">{children}</div>
    </>
  );
}
