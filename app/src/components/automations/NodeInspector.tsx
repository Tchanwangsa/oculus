import { useState } from "react";
import { CaretDown, Plus, Trash, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Textarea } from "@/components/ui/textarea";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  aiSlots,
  bindingLabel,
  branchesOf,
  CALENDAR_KINDS,
  EVENT_KINDS,
  EVENT_LABELS,
  GLOBAL_VARS,
  INBOX_SCOPES,
  incomingLinks,
  inputPorts,
  type AutomationGraph,
  type AutomationNode,
  type Branch,
} from "@/lib/automations";
import { SPEC_BY_KIND } from "@/components/automations/catalog";
import RuleEditor from "@/components/automations/RuleEditor";
import { useSubjects } from "@/hooks/useSubjects";
import { displayCode, displayName, humanizeSlug } from "@/lib/format";

const DAYS = ["S", "M", "T", "W", "T", "F", "S"];

/** The categories the scraper files things under. Labels are humanised rather
 *  than written out, so a new category shows up readable without a table. */
const FILE_CATEGORIES = [
  "announcement", "assignment", "ed", "file", "home",
  "image", "module", "page", "quiz", "syllabus",
];

/** A slot's name in the graph is derived from its label, so `{{…}}` in the
 *  prompt reads as the thing it names rather than as an id. */
const slugSlot = (label: string) =>
  label.trim().toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "");

// ── Shared field helpers ─────────────────────────────────────────────────────

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label className="text-[11px] font-medium text-muted-foreground">{label}</Label>
      {children}
    </div>
  );
}

/** The `{{…}}` names this node's text fields understand — its own input slots
 *  plus the globals — clickable to insert. Every field that goes through
 *  `render` at runtime gets one; a field that does not, does not. */
function VarChips({ names, onInsert }: { names: string[]; onInsert: (v: string) => void }) {
  return (
    <div className="flex flex-wrap gap-1 pt-0.5">
      {names.map((v) => (
        <button
          key={v}
          type="button"
          onClick={() => onInsert(`{{${v}}}`)}
          className="rounded border border-border-subtle bg-surface px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground transition-colors hover:border-border hover:text-foreground"
        >
          {v}
        </button>
      ))}
    </div>
  );
}

/**
 * A whole number that is stored, not typed.
 *
 * The box has to survive being emptied mid-edit — "" is a keystroke, not a
 * value of zero — and a pasted 999999999 must not become a window nothing can
 * answer. So the draft stays local until it parses, and only a clamped integer
 * ever reaches the graph.
 */
function NumberField({
  value,
  min,
  max,
  onChange,
  className = "",
}: {
  value: number;
  min: number;
  max: number;
  onChange: (n: number) => void;
  className?: string;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  return (
    <Input
      type="number"
      min={min}
      max={max}
      value={draft ?? String(value)}
      onChange={(e) => {
        const raw = e.target.value;
        setDraft(raw);
        const n = Math.floor(Number(raw));
        if (raw.trim() !== "" && Number.isFinite(n)) onChange(Math.min(max, Math.max(min, n)));
      }}
      onBlur={() => setDraft(null)}
      className={`h-7 text-xs tabular-nums ${className}`}
    />
  );
}

/** Subjects as a checkbox list. An empty selection is "every subject" — the
 *  automation should widen as you enrol, not quietly read nothing. */
function SubjectPicker({ ids, onChange }: { ids: number[]; onChange: (ids: number[]) => void }) {
  const { subjects } = useSubjects();
  const chosen = subjects.filter((s) => ids.includes(s.id));
  const summary =
    ids.length === 0
      ? "Every subject"
      : chosen.length > 0
        ? chosen.map((s) => displayCode(s.code)).join(", ")
        : `${ids.length} subjects`;

  const toggle = (id: number, on: boolean) =>
    onChange(on ? [...ids, id] : ids.filter((i) => i !== id));

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" className="h-7 w-full justify-between px-2 text-xs font-normal">
          <span className="truncate">{summary}</span>
          <CaretDown size={11} className="shrink-0 text-muted-foreground" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" className="max-h-72 w-64 overflow-y-auto p-1">
        <button
          type="button"
          onClick={() => onChange([])}
          className="flex w-full items-center rounded-md px-2 py-1.5 text-left text-xs text-foreground transition-colors hover:bg-accent"
        >
          Every subject
        </button>
        {subjects.map((s) => (
          <label
            key={s.id}
            className="flex cursor-pointer items-start gap-2 rounded-md px-2 py-1.5 transition-colors hover:bg-accent"
          >
            <Checkbox
              checked={ids.includes(s.id)}
              onCheckedChange={(v) => toggle(s.id, v === true)}
              className="mt-0.5"
            />
            <span className="min-w-0">
              <span className="block text-xs text-foreground">{displayCode(s.code)}</span>
              <span className="block truncate text-[11px] text-muted-foreground">
                {displayName(s.name, s.code)}
              </span>
            </span>
          </label>
        ))}
      </PopoverContent>
    </Popover>
  );
}

/** One subject, or none at all — what an event written into the calendar hangs
 *  off. */
function SubjectSelect({
  id,
  onChange,
}: {
  id: number | null;
  onChange: (id: number | null) => void;
}) {
  const { subjects } = useSubjects();
  return (
    <Select
      value={id == null ? "none" : String(id)}
      onValueChange={(v) => onChange(v === "none" ? null : Number(v))}
    >
      <SelectTrigger size="sm" className="h-7 w-full text-xs">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="none">No subject</SelectItem>
        {subjects.map((s) => (
          <SelectItem key={s.id} value={String(s.id)}>
            {displayCode(s.code)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

/** What every per-kind section is handed: the node's config and the writers
 *  that push straight through to the graph. */
interface SectionProps {
  c: Record<string, any>;
  set: (patch: Record<string, any>) => void;
  append: (key: string, text: string) => void;
  vars: string[];
}

// ── trigger.schedule ─────────────────────────────────────────────────────────

/** Units the stored `intervalMinutes` is shown in. Ordered smallest first;
 *  the display unit is the largest that divides the stored value evenly. */
const INTERVAL_UNITS = [
  { id: "minutes", label: "minute", minutes: 1 },
  { id: "hours", label: "hour", minutes: 60 },
  { id: "days", label: "day", minutes: 1440 },
  { id: "weeks", label: "week", minutes: 10080 },
] as const;

type IntervalUnit = (typeof INTERVAL_UNITS)[number]["id"];

/** Past a year an "interval" is a date, not a rhythm — and the tick that has
 *  to hold the anchor is only 30 seconds wide. */
const MAX_INTERVAL_MINUTES = 52 * 7 * 1440;

function splitInterval(stored: unknown): { amount: number; unit: IntervalUnit } {
  const raw = Math.floor(Number(stored));
  const m = Number.isFinite(raw) ? Math.min(MAX_INTERVAL_MINUTES, Math.max(1, raw)) : 360;
  for (let i = INTERVAL_UNITS.length - 1; i >= 0; i--) {
    const u = INTERVAL_UNITS[i];
    if (m % u.minutes === 0) return { amount: m / u.minutes, unit: u.id };
  }
  return { amount: m, unit: "minutes" };
}

/** Amount plus unit over **one** stored fact: `intervalMinutes`. The unit is
 *  derived on read and multiplied back on write, so there is nothing to keep
 *  in step and nothing to migrate. */
function IntervalField({ minutes, onChange }: { minutes: number; onChange: (m: number) => void }) {
  const { amount, unit } = splitInterval(minutes);
  const per = INTERVAL_UNITS.find((u) => u.id === unit)!.minutes;
  const maxAmount = Math.floor(MAX_INTERVAL_MINUTES / per);

  return (
    <div className="flex gap-1.5">
      <NumberField
        value={amount}
        min={1}
        max={maxAmount}
        onChange={(n) => onChange(Math.min(MAX_INTERVAL_MINUTES, n * per))}
        className="w-16 shrink-0"
      />
      <Select
        value={unit}
        onValueChange={(v) => {
          const next = INTERVAL_UNITS.find((u) => u.id === v)!.minutes;
          onChange(Math.min(MAX_INTERVAL_MINUTES, Math.max(1, amount * next)));
        }}
      >
        <SelectTrigger size="sm" className="h-7 flex-1 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {INTERVAL_UNITS.map((u) => (
            <SelectItem key={u.id} value={u.id}>
              {/* The trigger renders the chosen item, so pluralising here is
                  what stops "Every 1 weeks". */}
              {amount === 1 ? u.label : `${u.label}s`}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function ScheduleSection({ c, set }: SectionProps) {
  return (
    <>
      <Field label="Repeats">
        <Select value={c.scheduleKind ?? "daily"} onValueChange={(v) => set({ scheduleKind: v })}>
          <SelectTrigger size="sm" className="h-7 w-full text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="daily">Every day</SelectItem>
            <SelectItem value="weekly">On chosen days</SelectItem>
            <SelectItem value="interval">On an interval</SelectItem>
          </SelectContent>
        </Select>
      </Field>

      {c.scheduleKind === "weekly" && (
        <Field label="Days">
          <ToggleGroup
            type="multiple"
            size="sm"
            variant="outline"
            value={(c.days ?? []).map(String)}
            onValueChange={(v: string[]) => set({ days: v.map(Number).sort() })}
            className="w-full"
          >
            {DAYS.map((d, i) => (
              <ToggleGroupItem key={i} value={String(i)} className="flex-1 text-[11px]">
                {d}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </Field>
      )}

      {c.scheduleKind === "interval" ? (
        <Field label="Every">
          <IntervalField
            minutes={c.intervalMinutes ?? 360}
            onChange={(m) => set({ intervalMinutes: m })}
          />
        </Field>
      ) : (
        <Field label="At">
          <input
            type="time"
            value={c.timeOfDay ?? "09:00"}
            onChange={(e) => set({ timeOfDay: e.target.value })}
            className="h-7 w-full rounded-md border border-input bg-transparent px-2 text-xs tabular-nums text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          />
        </Field>
      )}
    </>
  );
}

// ── condition.if ─────────────────────────────────────────────────────────────

function ConditionSection({
  node,
  c,
  set,
}: SectionProps & { node: AutomationNode }) {
  return (
    <RuleEditor
      branches={branchesOf(node)}
      otherwise={c.otherwise !== false}
      onBranches={(branches: Branch[]) => set({ branches })}
      onOtherwise={(otherwise) => set({ otherwise })}
    />
  );
}

// ── source.* ─────────────────────────────────────────────────────────────────

function InboxSourceSection({ c, set }: SectionProps) {
  return (
    <>
      <Field label="Read">
        <Select value={c.scope ?? "unread"} onValueChange={(v) => set({ scope: v })}>
          <SelectTrigger size="sm" className="h-7 w-full text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {Object.entries(INBOX_SCOPES).map(([id, label]) => (
              <SelectItem key={id} value={id}>
                {label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>
      <Field label="From the last (days)">
        <NumberField value={c.days ?? 7} min={1} max={3650} onChange={(days) => set({ days })} />
      </Field>
      <Field label="At most (items)">
        <NumberField value={c.limit ?? 20} min={1} max={500} onChange={(limit) => set({ limit })} />
      </Field>
    </>
  );
}

function CalendarSourceSection({ c, set }: SectionProps) {
  const kinds: string[] = Array.isArray(c.kinds) ? c.kinds : ["due"];
  return (
    <>
      <Field label="Include">
        <ToggleGroup
          type="multiple"
          size="sm"
          variant="outline"
          value={kinds}
          onValueChange={(v: string[]) => set({ kinds: v })}
          className="w-full"
        >
          {Object.entries(CALENDAR_KINDS).map(([id, label]) => (
            <ToggleGroupItem key={id} value={id} className="flex-1 text-[11px]">
              {label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </Field>
      {kinds.length === 0 && (
        <p className="text-[11px] leading-relaxed text-muted-foreground">
          Nothing ticked, so this node reads no events.
        </p>
      )}
      <Field label="Next (days)">
        <NumberField value={c.days ?? 7} min={1} max={3650} onChange={(days) => set({ days })} />
      </Field>
      <Field label="Subjects">
        <SubjectPicker
          ids={Array.isArray(c.subjectIds) ? c.subjectIds : []}
          onChange={(subjectIds) => set({ subjectIds })}
        />
      </Field>
    </>
  );
}

function FilesSourceSection({ c, set }: SectionProps) {
  return (
    <>
      <Field label="From the last (days)">
        <NumberField value={c.days ?? 7} min={1} max={3650} onChange={(days) => set({ days })} />
      </Field>
      <Field label="Subjects">
        <SubjectPicker
          ids={Array.isArray(c.subjectIds) ? c.subjectIds : []}
          onChange={(subjectIds) => set({ subjectIds })}
        />
      </Field>
      <Field label="Category">
        <Select
          value={c.category ?? "any"}
          onValueChange={(v) => set({ category: v === "any" ? null : v })}
        >
          <SelectTrigger size="sm" className="h-7 w-full text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="any">Any category</SelectItem>
            {FILE_CATEGORIES.map((cat) => (
              <SelectItem key={cat} value={cat}>
                {humanizeSlug(cat)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>
      <Field label="At most (files)">
        <NumberField value={c.limit ?? 50} min={1} max={1000} onChange={(limit) => set({ limit })} />
      </Field>
    </>
  );
}

// ── action.* ─────────────────────────────────────────────────────────────────

function AiSection({ c, set, append, vars }: SectionProps) {
  return (
    <>
      <Field label="Inputs">
        <div className="flex flex-col gap-1">
          {aiSlots(c).map((p, i) => (
            <div key={i} className="flex items-center gap-1">
              <Input
                value={p.label}
                onChange={(e) => {
                  // The id is fixed at creation: renaming a slot must not
                  // silently unplug the wire already in it.
                  const next = aiSlots(c).map((s2, j) =>
                    j === i
                      ? { id: s2.id, label: e.target.value }
                      : { id: s2.id, label: s2.label },
                  );
                  set({ slots: next });
                }}
                className="h-7 flex-1 text-xs"
              />
              <span className="font-mono text-[10px] text-muted-foreground">{`{{${p.id}}}`}</span>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Remove input"
                onClick={() =>
                  set({ slots: aiSlots(c).filter((_, j) => j !== i).map((s2) => ({ id: s2.id, label: s2.label })) })
                }
                className="shrink-0 text-muted-foreground/60"
              >
                <X size={11} />
              </Button>
            </div>
          ))}
          <Button
            variant="ghost"
            size="sm"
            className="h-6 justify-start px-1 text-[11px] text-muted-foreground"
            onClick={() => {
              const existing = aiSlots(c);
              const label = `Input ${existing.length + 1}`;
              const id = slugSlot(label) || `input_${existing.length + 1}`;
              set({ slots: [...existing.map((s2) => ({ id: s2.id, label: s2.label })), { id, label }] });
            }}
          >
            <Plus size={11} /> Add input
          </Button>
        </div>
      </Field>
      <p className="text-[11px] leading-relaxed text-muted-foreground">
        Each input is a slot on the node's left edge. Wire something into it,
        then name it in the prompt.
      </p>
      <Field label="Prompt">
        <Textarea
          value={c.prompt ?? ""}
          onChange={(e) => set({ prompt: e.target.value })}
          rows={8}
          className="text-xs leading-relaxed"
        />
      </Field>
      <VarChips names={vars} onInsert={(v) => append("prompt", v)} />
      <Field label="System prompt (optional)">
        <Textarea
          value={c.system ?? ""}
          onChange={(e) => set({ system: e.target.value })}
          rows={3}
          className="text-xs leading-relaxed"
        />
      </Field>
      <VarChips names={vars} onInsert={(v) => append("system", v)} />
      <p className="text-[11px] leading-relaxed text-muted-foreground">
        Uses the summary model from Settings → AI. The reply leaves by the
        node's "Reply" port — wire it into whatever should receive it.
      </p>
    </>
  );
}

/** The Inbox item and the desktop notification share a shape: a heading and a
 *  body, both rendered as templates. */
function MessageSection({ node, c, set, append, vars }: SectionProps & { node: AutomationNode }) {
  const isInbox = node.kind === "action.inbox";
  return (
    <>
      <Field label={isInbox ? "Subject" : "Title"}>
        <Input
          value={c.title ?? ""}
          onChange={(e) => set({ title: e.target.value })}
          className="h-7 text-xs"
        />
      </Field>
      <VarChips names={vars} onInsert={(v) => append("title", v)} />
      <Field label={isInbox ? "Body (markdown)" : "Body"}>
        <Textarea
          value={c.body ?? ""}
          onChange={(e) => set({ body: e.target.value })}
          rows={isInbox ? 8 : 4}
          className="text-xs leading-relaxed"
        />
      </Field>
      <VarChips names={vars} onInsert={(v) => append("body", v)} />
      {isInbox && (
        <p className="text-[11px] leading-relaxed text-muted-foreground">
          Summaries wired in become one Inbox item with a row per file, each
          linking to the file. Anything else is written as a single note using
          the body above.
        </p>
      )}
    </>
  );
}

function CalendarActionSection({ c, set, append, vars }: SectionProps) {
  const when = c.when === "template" ? "template" : "offset";
  return (
    <>
      <Field label="Title">
        <Input
          value={c.title ?? ""}
          onChange={(e) => set({ title: e.target.value })}
          className="h-7 text-xs"
        />
      </Field>
      <VarChips names={vars} onInsert={(v) => append("title", v)} />

      <Field label="Kind">
        <Select value={c.kind ?? "note"} onValueChange={(v) => set({ kind: v })}>
          <SelectTrigger size="sm" className="h-7 w-full text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {Object.entries(EVENT_KINDS).map(([id, label]) => (
              <SelectItem key={id} value={id}>
                {label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>

      <Field label="When">
        <Select value={when} onValueChange={(v) => set({ when: v })}>
          <SelectTrigger size="sm" className="h-7 w-full text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="offset">A number of days from now</SelectItem>
            <SelectItem value="template">A date from the text</SelectItem>
          </SelectContent>
        </Select>
      </Field>

      {when === "offset" ? (
        <Field label="Days from now">
          <NumberField
            value={c.offsetDays ?? 1}
            min={0}
            max={3650}
            onChange={(offsetDays) => set({ offsetDays })}
          />
        </Field>
      ) : (
        <>
          <Field label="Date">
            <Input
              value={c.at ?? ""}
              onChange={(e) => set({ at: e.target.value })}
              className="h-7 text-xs"
            />
          </Field>
          <VarChips names={vars} onInsert={(v) => append("at", v)} />
          <p className="text-[11px] leading-relaxed text-muted-foreground">
            A plain date (2026-08-19) takes the time below; anything that reads
            as a full date and time brings its own.
          </p>
        </>
      )}

      <Field label="Time of day">
        <input
          type="time"
          value={c.timeOfDay ?? "09:00"}
          onChange={(e) => set({ timeOfDay: e.target.value })}
          className="h-7 w-full rounded-md border border-input bg-transparent px-2 text-xs tabular-nums text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        />
      </Field>

      <Field label="Subject">
        <SubjectSelect
          id={typeof c.subjectId === "number" ? c.subjectId : null}
          onChange={(subjectId) => set({ subjectId })}
        />
      </Field>

      <Field label="Notes">
        <Textarea
          value={c.notes ?? ""}
          onChange={(e) => set({ notes: e.target.value })}
          rows={4}
          className="text-xs leading-relaxed"
        />
      </Field>
      <VarChips names={vars} onInsert={(v) => append("notes", v)} />
    </>
  );
}

function SummariseSection({ c, set }: SectionProps) {
  return (
    <>
      <Field label="Ask of each file (optional)">
        <Textarea
          value={c.instruction ?? ""}
          onChange={(e) => set({ instruction: e.target.value })}
          rows={4}
          placeholder="Summarise this in 2-3 sentences for a study inbox."
          className="text-xs leading-relaxed"
        />
      </Field>
      <p className="text-[11px] leading-relaxed text-muted-foreground">
        Runs once per file wired in, waiting up to 15 minutes for the parser.
        The work happens where the result is used: wire the summaries into the
        Inbox and the item appears straight away with a row per file, filling
        in as each one lands.
      </p>
    </>
  );
}

/**
 * Config for the selected node.
 *
 * Writes go straight through `onChange` — the canvas owns the graph and
 * debounces the save, so there is no local draft state to fall out of sync.
 * The exceptions are numeric boxes, which need a draft to survive being
 * emptied mid-edit (see `NumberField`).
 */
export default function NodeInspector({
  node,
  graph,
  onChange,
  onDelete,
}: {
  node: AutomationNode;
  graph: AutomationGraph;
  onChange: (config: Record<string, any>) => void;
  onDelete: () => void;
}) {
  const spec = SPEC_BY_KIND[node.kind];
  const c = node.config ?? {};
  const set = (patch: Record<string, any>) => onChange({ ...c, ...patch });
  const append = (key: string, text: string) => set({ [key]: `${c[key] ?? ""}${text}` });

  const slots = inputPorts(node);
  const wires = incomingLinks(graph, node.id);
  // Exactly what `render` resolves: this node's own input slots, plus globals.
  const vars = [...slots.map((p) => p.id), ...GLOBAL_VARS];
  const section: SectionProps = { c, set, append, vars };

  return (
    <div className="flex h-full w-[300px] shrink-0 flex-col border-l border-border-subtle bg-background">
      <div className="flex items-center justify-between gap-2 border-b border-border-subtle px-4 py-3">
        <div className="min-w-0">
          <div className="truncate text-xs font-medium text-foreground">{spec?.title}</div>
          <div className="truncate text-[11px] text-muted-foreground">{spec?.blurb}</div>
        </div>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Delete node"
          onClick={onDelete}
          className="shrink-0 text-muted-foreground/60 hover:text-destructive"
        >
          <Trash size={13} />
        </Button>
      </div>

      <div className="flex flex-1 flex-col gap-4 overflow-y-auto px-4 py-4">
        {slots.length > 0 && (
          <Field label="Wired in">
            <div className="flex flex-col gap-1">
              {slots.map((p) => {
                const feeds = wires.filter((l) => l.toPort === p.id);
                return (
                  <div key={p.id} className="flex flex-col gap-0.5 rounded-md border border-border-subtle px-2 py-1.5">
                    <span className="font-mono text-[10px] text-foreground">{`{{${p.id}}}`}</span>
                    {feeds.length === 0 ? (
                      <span className="text-[11px] text-muted-foreground/70">
                        Nothing connected
                      </span>
                    ) : (
                      feeds.map((l) => (
                        <span key={`${l.from}:${l.fromPort}`} className="truncate text-[11px] text-muted-foreground">
                          ← {bindingLabel(graph, l)}
                        </span>
                      ))
                    )}
                  </div>
                );
              })}
            </div>
          </Field>
        )}

        {node.kind === "trigger.schedule" && <ScheduleSection {...section} />}

        {node.kind === "trigger.event" && (
          <Field label="Event">
            <Select value={c.event ?? "sync-complete"} onValueChange={(v) => set({ event: v })}>
              <SelectTrigger size="sm" className="h-7 w-full text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {Object.entries(EVENT_LABELS).map(([id, label]) => (
                  <SelectItem key={id} value={id}>
                    When {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
        )}

        {node.kind === "condition.if" && <ConditionSection node={node} {...section} />}

        {node.kind === "source.inbox" && <InboxSourceSection {...section} />}
        {node.kind === "source.calendar" && <CalendarSourceSection {...section} />}
        {node.kind === "source.files" && <FilesSourceSection {...section} />}

        {node.kind === "action.ai" && <AiSection {...section} />}

        {(node.kind === "action.inbox" || node.kind === "action.notify") && (
          <MessageSection node={node} {...section} />
        )}

        {node.kind === "action.calendar" && <CalendarActionSection {...section} />}

        {node.kind === "action.sync" && (
          <p className="text-[11px] leading-relaxed text-muted-foreground">
            Syncs every subject ticked on the Sync page. Nothing to configure
            here — change the selection there.
          </p>
        )}

        {node.kind === "action.summarise" && <SummariseSection {...section} />}
      </div>
    </div>
  );
}
