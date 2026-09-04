import { CaretDown, CaretUp, Plus, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
  CLOCK_FIELDS,
  INPUT_FIELDS,
  RULE_OPS,
  isUnaryOp,
  newBranch,
  type Branch,
  type Operand,
  type Rule,
  type RuleOp,
} from "@/lib/automations";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

/** Weekday numbers are Sunday-first to match `Date.getDay()`, which is what a
 *  `clock` operand resolves to — so the picker has to name them, not count. */
const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];

/** An operand as one select value: the kind, plus the field it reads when the
 *  kind has one. Literals carry theirs in a second control. */
const operandKey = (o: Operand | undefined): string => {
  if (o?.kind === "input") return `input.${o.field ?? "count"}`;
  if (o?.kind === "clock") return `clock.${o.clock ?? "hour"}`;
  return o?.kind ?? "number";
};

/** Rebuilding from the key keeps the literal the user already typed, so
 *  flipping text → number → text does not wipe it. */
function operandFromKey(key: string, prev: Operand | undefined): Operand {
  const [kind, field] = key.split(".");
  if (kind === "input") return { kind: "input", field: field as Operand["field"] };
  if (kind === "clock") return { kind: "clock", clock: field as Operand["clock"] };
  if (kind === "number") return { kind: "number", n: prev?.n ?? 0 };
  return { kind: "text", text: prev?.text ?? "" };
}

/** What a clock operand on the left turns the right-hand number into: bare
 *  0-6 is unreadable, "Tuesday" is not. */
const namedNumbers = (left: Operand | undefined): string[] | null => {
  if (left?.kind !== "clock") return null;
  if (left.clock === "weekday") return WEEKDAYS;
  if (left.clock === "month") return MONTHS;
  return null;
};

function OperandSelect({
  value,
  side,
  onChange,
}: {
  value: Operand | undefined;
  side: "left" | "right";
  onChange: (o: Operand) => void;
}) {
  return (
    <Select value={operandKey(value)} onValueChange={(k) => onChange(operandFromKey(k, value))}>
      <SelectTrigger size="sm" className="h-7 w-full text-xs">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {side === "right" && (
          <SelectGroup>
            <SelectItem value="number">A number</SelectItem>
            <SelectItem value="text">Some text</SelectItem>
          </SelectGroup>
        )}
        {side === "left" && (
          <SelectGroup>
            <SelectLabel>What came in</SelectLabel>
            {Object.entries(INPUT_FIELDS).map(([id, label]) => (
              <SelectItem key={id} value={`input.${id}`}>
                {label}
              </SelectItem>
            ))}
          </SelectGroup>
        )}
        <SelectGroup>
          <SelectLabel>The clock</SelectLabel>
          {Object.entries(CLOCK_FIELDS).map(([id, label]) => (
            <SelectItem key={id} value={`clock.${id}`}>
              {label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  );
}

/** The literal half of a right-hand operand — nothing at all when the operand
 *  is a clock fact, since that one reads itself. */
function OperandLiteral({
  value,
  left,
  onChange,
}: {
  value: Operand;
  left: Operand | undefined;
  onChange: (o: Operand) => void;
}) {
  if (value.kind === "text") {
    return (
      <Input
        value={value.text ?? ""}
        onChange={(e) => onChange({ ...value, text: e.target.value })}
        className="h-7 text-xs"
      />
    );
  }
  if (value.kind !== "number") return null;

  const names = namedNumbers(left);
  if (names) {
    // Months are 1-12, weekdays 0-6 — the offset is the whole difference.
    const base = left?.clock === "month" ? 1 : 0;
    return (
      <Select
        value={String(value.n ?? base)}
        onValueChange={(v) => onChange({ ...value, n: Number(v) })}
      >
        <SelectTrigger size="sm" className="h-7 w-full text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {names.map((label, i) => (
            <SelectItem key={label} value={String(i + base)}>
              {label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  }

  return (
    <Input
      type="number"
      value={String(value.n ?? 0)}
      onChange={(e) => {
        const n = Number(e.target.value);
        onChange({ ...value, n: Number.isFinite(n) ? n : 0 });
      }}
      className="h-7 text-xs tabular-nums"
    />
  );
}

function RuleRow({
  rule,
  onChange,
  onRemove,
}: {
  rule: Rule;
  onChange: (r: Rule) => void;
  onRemove: () => void;
}) {
  const unary = isUnaryOp(rule.op);
  return (
    <div className="flex flex-col gap-1 rounded-md border border-border-subtle bg-surface/40 p-1.5">
      <div className="flex items-start gap-1">
        <div className="min-w-0 flex-1">
          <OperandSelect value={rule.left} side="left" onChange={(left) => onChange({ ...rule, left })} />
        </div>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Remove check"
          onClick={onRemove}
          className="mt-0.5 shrink-0 text-muted-foreground/60 hover:text-destructive"
        >
          <X size={11} />
        </Button>
      </div>

      <Select value={rule.op} onValueChange={(op) => onChange({ ...rule, op: op as RuleOp })}>
        <SelectTrigger size="sm" className="h-7 w-full text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {Object.entries(RULE_OPS).map(([id, label]) => (
            <SelectItem key={id} value={id}>
              {label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {/* A unary op reads the left side alone: a right-hand control here would
          be one nothing downstream ever looks at. */}
      {!unary && (
        <div className="flex flex-col gap-1">
          <OperandSelect
            value={rule.right}
            side="right"
            onChange={(right) => onChange({ ...rule, right })}
          />
          <OperandLiteral
            value={rule.right ?? { kind: "number", n: 0 }}
            left={rule.left}
            onChange={(right) => onChange({ ...rule, right })}
          />
        </div>
      )}
    </div>
  );
}

function BranchCard({
  branch,
  index,
  count,
  onChange,
  onRemove,
  onMove,
}: {
  branch: Branch;
  index: number;
  count: number;
  onChange: (b: Branch) => void;
  onRemove: () => void;
  onMove: (dir: -1 | 1) => void;
}) {
  const setRules = (rules: Rule[]) => onChange({ ...branch, rules });

  return (
    <div className="flex flex-col gap-1.5 rounded-md border border-border px-2 py-2">
      <div className="flex items-center gap-1">
        <span className="w-4 shrink-0 text-center font-mono text-[10px] text-muted-foreground">
          {index + 1}
        </span>
        <Input
          value={branch.label}
          maxLength={24}
          placeholder="Branch"
          onChange={(e) => onChange({ ...branch, label: e.target.value })}
          // The label is the port label on the canvas, so an empty one would
          // leave a nameless output; fall back rather than let that happen.
          onBlur={() => {
            if (!branch.label.trim()) onChange({ ...branch, label: `Branch ${index + 1}` });
          }}
          className="h-7 min-w-0 flex-1 text-xs"
        />
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Move up"
          disabled={index === 0}
          onClick={() => onMove(-1)}
          className="shrink-0 text-muted-foreground/60"
        >
          <CaretUp size={11} />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Move down"
          disabled={index === count - 1}
          onClick={() => onMove(1)}
          className="shrink-0 text-muted-foreground/60"
        >
          <CaretDown size={11} />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="Remove branch"
          onClick={onRemove}
          className="shrink-0 text-muted-foreground/60 hover:text-destructive"
        >
          <X size={11} />
        </Button>
      </div>

      {branch.rules.length > 1 && (
        <ToggleGroup
          type="single"
          size="sm"
          variant="outline"
          value={branch.match}
          onValueChange={(v: string) => v && onChange({ ...branch, match: v as Branch["match"] })}
          className="w-full"
        >
          <ToggleGroupItem value="all" className="flex-1 text-[11px]">
            All of these
          </ToggleGroupItem>
          <ToggleGroupItem value="any" className="flex-1 text-[11px]">
            Any of these
          </ToggleGroupItem>
        </ToggleGroup>
      )}

      {branch.rules.length === 0 ? (
        <p className="text-[11px] text-muted-foreground/70">
          No checks — this branch takes everything that reaches it.
        </p>
      ) : (
        branch.rules.map((r, i) => (
          <RuleRow
            key={i}
            rule={r}
            onChange={(next) => setRules(branch.rules.map((r2, j) => (j === i ? next : r2)))}
            onRemove={() => setRules(branch.rules.filter((_, j) => j !== i))}
          />
        ))
      )}

      <Button
        variant="ghost"
        size="sm"
        className="h-6 justify-start px-1 text-[11px] text-muted-foreground"
        onClick={() =>
          setRules([
            ...branch.rules,
            { left: { kind: "input", field: "count" }, op: "gt", right: { kind: "number", n: 0 } },
          ])
        }
      >
        <Plus size={11} /> Add check
      </Button>
    </div>
  );
}

/**
 * The branch list of a `condition.if` node.
 *
 * This is a router, not a fan-out: branches are tested top to bottom and the
 * first one that passes takes the value, so the order on screen is the order
 * at runtime — which is why every branch can be moved, and why they are
 * numbered rather than merely listed.
 */
export default function RuleEditor({
  branches,
  otherwise,
  onBranches,
  onOtherwise,
}: {
  branches: Branch[];
  otherwise: boolean;
  onBranches: (b: Branch[]) => void;
  onOtherwise: (v: boolean) => void;
}) {
  const move = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= branches.length) return;
    const next = [...branches];
    [next[i], next[j]] = [next[j], next[i]];
    onBranches(next);
  };

  return (
    <div className="flex flex-col gap-2">
      <Label className="text-[11px] font-medium text-muted-foreground">Branches</Label>
      <p className="text-[11px] leading-relaxed text-muted-foreground">
        Checked top to bottom. The first branch that passes takes the value and
        the rest never fire.
      </p>

      {branches.map((b, i) => (
        <BranchCard
          key={b.id}
          branch={b}
          index={i}
          count={branches.length}
          onChange={(next) => onBranches(branches.map((b2, j) => (j === i ? next : b2)))}
          onRemove={() => onBranches(branches.filter((_, j) => j !== i))}
          onMove={(dir) => move(i, dir)}
        />
      ))}

      <Button
        variant="ghost"
        size="sm"
        className="h-6 justify-start px-1 text-[11px] text-muted-foreground"
        onClick={() => onBranches([...branches, newBranch(`Branch ${branches.length + 1}`)])}
      >
        <Plus size={11} /> Add branch
      </Button>

      <div className="flex items-center justify-between gap-2 rounded-md border border-border-subtle px-2 py-1.5">
        <span className="text-[11px] text-foreground">Otherwise</span>
        <Switch size="sm" checked={otherwise} onCheckedChange={onOtherwise} />
      </div>
      <p className="text-[11px] leading-relaxed text-muted-foreground">
        Adds a port for whatever matched no branch. Off, and unmatched values
        stop here.
      </p>
    </div>
  );
}
